use crate::server::SshKey;
use log::info;
use std::collections::HashMap;
use std::collections::HashSet;
use tokio_postgres::Client;

// Structure for storing key processing statistics
pub struct KeyInsertStats {
    pub total: usize,                   // Total number of received keys
    pub inserted: usize,                // Number of new keys
    pub unchanged: usize,               // Number of unchanged keys
    pub key_id_map: Vec<(SshKey, i32)>, // Mapping of keys to their IDs in the database
}

pub async fn initialize_db_schema(client: &Client) -> Result<(), tokio_postgres::Error> {
    info!("Checking and initializing database schema if needed");

    // Check if tables exist by querying information_schema
    let tables_exist = client
        .query(
            "SELECT EXISTS (
                SELECT FROM information_schema.tables 
                WHERE table_schema = 'public' 
                AND table_name = 'keys'
            ) AND EXISTS (
                SELECT FROM information_schema.tables 
                WHERE table_schema = 'public' 
                AND table_name = 'flows'
            )",
            &[],
        )
        .await?
        .get(0)
        .map(|row| row.get::<_, bool>(0))
        .unwrap_or(false);

    if !tables_exist {
        info!("Database schema doesn't exist. Creating tables...");

        // Create the keys table
        client
            .execute(
                "CREATE TABLE IF NOT EXISTS public.keys (
                    key_id SERIAL PRIMARY KEY,
                    host VARCHAR(255) NOT NULL,
                    key TEXT NOT NULL,
                    updated TIMESTAMP WITH TIME ZONE NOT NULL,
                    deprecated BOOLEAN NOT NULL DEFAULT FALSE,
                    CONSTRAINT unique_host_key UNIQUE (host, key)
                )",
                &[],
            )
            .await?;

        // Create the flows table
        client
            .execute(
                "CREATE TABLE IF NOT EXISTS public.flows (
                    flow_id SERIAL PRIMARY KEY,
                    name VARCHAR(255) NOT NULL,
                    key_id INTEGER NOT NULL,
                    CONSTRAINT fk_key
                        FOREIGN KEY(key_id)
                        REFERENCES public.keys(key_id)
                        ON DELETE CASCADE,
                    CONSTRAINT unique_flow_key UNIQUE (name, key_id)
                )",
                &[],
            )
            .await?;

        // Create an index for faster lookups
        client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_flows_name ON public.flows(name)",
                &[],
            )
            .await?;

        info!("Database schema created successfully");
    } else {
        info!("Database schema already exists");
        
        // Check if deprecated column exists, add it if missing (migration)
        let column_exists = client
            .query(
                "SELECT EXISTS (
                    SELECT FROM information_schema.columns 
                    WHERE table_schema = 'public' 
                    AND table_name = 'keys' 
                    AND column_name = 'deprecated'
                )",
                &[],
            )
            .await?
            .get(0)
            .map(|row| row.get::<_, bool>(0))
            .unwrap_or(false);
        
        if !column_exists {
            info!("Adding deprecated column to existing keys table...");
            client
                .execute(
                    "ALTER TABLE public.keys ADD COLUMN deprecated BOOLEAN NOT NULL DEFAULT FALSE",
                    &[],
                )
                .await?;
            info!("Migration completed: deprecated column added");
        }
    }

    Ok(())
}

pub async fn batch_insert_keys(
    client: &Client,
    keys: &[SshKey],
) -> Result<KeyInsertStats, tokio_postgres::Error> {
    if keys.is_empty() {
        return Ok(KeyInsertStats {
            total: 0,
            inserted: 0,
            unchanged: 0,
            key_id_map: Vec::new(),
        });
    }

    // Prepare arrays for batch insertion
    let mut host_values: Vec<&str> = Vec::with_capacity(keys.len());
    let mut key_values: Vec<&str> = Vec::with_capacity(keys.len());

    for key in keys {
        host_values.push(&key.server);
        key_values.push(&key.public_key);
    }

    // First, check which keys already exist in the database (including deprecated status)
    let mut existing_keys = HashMap::new();
    let mut key_query = String::from("SELECT host, key, key_id, deprecated FROM public.keys WHERE ");

    for i in 0..keys.len() {
        if i > 0 {
            key_query.push_str(" OR ");
        }
        key_query.push_str(&format!("(host = ${} AND key = ${})", i * 2 + 1, i * 2 + 2));
    }

    let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        Vec::with_capacity(keys.len() * 2);
    for i in 0..keys.len() {
        params.push(&host_values[i]);
        params.push(&key_values[i]);
    }

    let rows = client.query(&key_query, &params[..]).await?;

    for row in rows {
        let host: String = row.get(0);
        let key: String = row.get(1);
        let key_id: i32 = row.get(2);
        let deprecated: bool = row.get(3);
        existing_keys.insert((host, key), (key_id, deprecated));
    }

    // Determine which keys need to be inserted and which already exist
    let mut keys_to_insert = Vec::new();
    let mut unchanged_keys = Vec::new();
    let mut ignored_deprecated = 0;

    for key in keys {
        let key_tuple = (key.server.clone(), key.public_key.clone());
        if let Some((key_id, is_deprecated)) = existing_keys.get(&key_tuple) {
            if *is_deprecated {
                // Ignore deprecated keys - don't add them to any flow
                ignored_deprecated += 1;
            } else {
                // Key exists and is not deprecated - add to unchanged
                unchanged_keys.push((key.clone(), *key_id));
            }
        } else {
            // Key doesn't exist - add to insert list
            keys_to_insert.push(key.clone());
        }
    }

    let mut inserted_keys = Vec::new();

    // If there are keys to insert, perform the insertion
    if !keys_to_insert.is_empty() {
        let mut insert_sql = String::from("INSERT INTO public.keys (host, key, updated) VALUES ");

        let mut insert_params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::new();
        let mut param_count = 1;

        for (i, key) in keys_to_insert.iter().enumerate() {
            if i > 0 {
                insert_sql.push_str(", ");
            }
            insert_sql.push_str(&format!("(${}, ${}, NOW())", param_count, param_count + 1));
            insert_params.push(&key.server);
            insert_params.push(&key.public_key);
            param_count += 2;
        }

        insert_sql.push_str(" RETURNING key_id, host, key");

        let inserted_rows = client.query(&insert_sql, &insert_params[..]).await?;

        for row in inserted_rows {
            let host: String = row.get(1);
            let key_text: String = row.get(2);
            let key_id: i32 = row.get(0);

            if let Some(orig_key) = keys_to_insert
                .iter()
                .find(|k| k.server == host && k.public_key == key_text)
            {
                inserted_keys.push((orig_key.clone(), key_id));
            }
        }
    }

    // Save the number of elements before combining
    let inserted_count = inserted_keys.len();
    let unchanged_count = unchanged_keys.len();

    // Combine results and generate statistics
    let mut key_id_map = Vec::with_capacity(unchanged_count + inserted_count);
    key_id_map.extend(unchanged_keys);
    key_id_map.extend(inserted_keys);

    let stats = KeyInsertStats {
        total: keys.len(),
        inserted: inserted_count,
        unchanged: unchanged_count,
        key_id_map,
    };

    info!(
        "Keys stats: received={}, new={}, unchanged={}, ignored_deprecated={}",
        stats.total, stats.inserted, stats.unchanged, ignored_deprecated
    );

    Ok(stats)
}

pub async fn batch_insert_flow_keys(
    client: &Client,
    flow_name: &str,
    key_ids: &[i32],
) -> Result<usize, tokio_postgres::Error> {
    if key_ids.is_empty() {
        info!("No keys to associate with flow '{}'", flow_name);
        return Ok(0);
    }

    // First, check which associations already exist
    let mut existing_query =
        String::from("SELECT key_id FROM public.flows WHERE name = $1 AND key_id IN (");

    for i in 0..key_ids.len() {
        if i > 0 {
            existing_query.push_str(", ");
        }
        existing_query.push_str(&format!("${}", i + 2));
    }
    existing_query.push_str(")");

    let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        Vec::with_capacity(key_ids.len() + 1);
    params.push(&flow_name);
    for key_id in key_ids {
        params.push(key_id);
    }

    let rows = client.query(&existing_query, &params[..]).await?;

    let mut existing_associations = HashSet::new();
    for row in rows {
        let key_id: i32 = row.get(0);
        existing_associations.insert(key_id);
    }

    // Filter only keys that are not yet associated with the flow
    let new_key_ids: Vec<&i32> = key_ids
        .iter()
        .filter(|&id| !existing_associations.contains(id))
        .collect();

    if new_key_ids.is_empty() {
        info!(
            "All {} keys are already associated with flow '{}'",
            key_ids.len(),
            flow_name
        );
        return Ok(0);
    }

    // Build SQL query with multiple values only for new associations
    let mut sql = String::from("INSERT INTO public.flows (name, key_id) VALUES ");

    for i in 0..new_key_ids.len() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("($1, ${})", i + 2));
    }

    sql.push_str(" ON CONFLICT (name, key_id) DO NOTHING");

    // Prepare parameters for the query
    let mut insert_params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        Vec::with_capacity(new_key_ids.len() + 1);
    insert_params.push(&flow_name);
    for key_id in &new_key_ids {
        insert_params.push(*key_id);
    }

    // Execute query
    let affected = client.execute(&sql, &insert_params[..]).await?;

    let affected_usize = affected as usize;

    info!(
        "Added {} new key-flow associations for flow '{}' (skipped {} existing)",
        affected_usize,
        flow_name,
        existing_associations.len()
    );

    Ok(affected_usize)
}

// Function to deprecate keys instead of deleting them
pub async fn deprecate_key_by_server(
    client: &Client,
    server_name: &str,
    flow_name: &str,
) -> Result<u64, tokio_postgres::Error> {
    // Update keys to deprecated status for the given server
    let affected = client
        .execute(
            "UPDATE public.keys 
             SET deprecated = TRUE, updated = NOW() 
             WHERE host = $1 
             AND key_id IN (
                 SELECT key_id FROM public.flows WHERE name = $2
             )",
            &[&server_name, &flow_name],
        )
        .await?;

    info!(
        "Deprecated {} key(s) for server '{}' in flow '{}'",
        affected, server_name, flow_name
    );

    Ok(affected)
}

// Function to restore deprecated key back to active
pub async fn restore_key_by_server(
    client: &Client,
    server_name: &str,
    flow_name: &str,
) -> Result<u64, tokio_postgres::Error> {
    // Update keys to active status for the given server in the flow
    let affected = client
        .execute(
            "UPDATE public.keys 
             SET deprecated = FALSE, updated = NOW() 
             WHERE host = $1 
             AND deprecated = TRUE
             AND key_id IN (
                 SELECT key_id FROM public.flows WHERE name = $2
             )",
            &[&server_name, &flow_name],
        )
        .await?;

    info!(
        "Restored {} key(s) for server '{}' in flow '{}'",
        affected, server_name, flow_name
    );

    Ok(affected)
}

// Function to permanently delete keys from database
pub async fn permanently_delete_key_by_server(
    client: &Client,
    server_name: &str,
    flow_name: &str,
) -> Result<u64, tokio_postgres::Error> {
    // First, find the key_ids for the given server in the flow
    let key_rows = client
        .query(
            "SELECT k.key_id FROM public.keys k 
             INNER JOIN public.flows f ON k.key_id = f.key_id 
             WHERE k.host = $1 AND f.name = $2",
            &[&server_name, &flow_name]
        )
        .await?;

    if key_rows.is_empty() {
        return Ok(0);
    }

    let key_ids: Vec<i32> = key_rows.iter().map(|row| row.get::<_, i32>(0)).collect();

    // Delete flow associations first
    let mut flow_delete_count = 0;
    for key_id in &key_ids {
        let deleted = client
            .execute(
                "DELETE FROM public.flows WHERE name = $1 AND key_id = $2",
                &[&flow_name, key_id],
            )
            .await?;
        flow_delete_count += deleted;
    }

    // Check if any of these keys are used in other flows
    let mut keys_to_delete = Vec::new();
    for key_id in &key_ids {
        let count: i64 = client
            .query_one(
                "SELECT COUNT(*) FROM public.flows WHERE key_id = $1",
                &[key_id],
            )
            .await?
            .get(0);

        if count == 0 {
            keys_to_delete.push(*key_id);
        }
    }

    // Permanently delete keys that are no longer referenced by any flow
    let mut total_deleted = 0;
    for key_id in keys_to_delete {
        let deleted = client
            .execute("DELETE FROM public.keys WHERE key_id = $1", &[&key_id])
            .await?;
        total_deleted += deleted;
    }

    info!(
        "Permanently deleted {} flow associations and {} orphaned keys for server '{}' in flow '{}'",
        flow_delete_count, total_deleted, server_name, flow_name
    );

    Ok(std::cmp::max(flow_delete_count, total_deleted))
}
