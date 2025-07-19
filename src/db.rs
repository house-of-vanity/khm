use crate::server::SshKey;
use log::{error, info, warn};
use std::collections::HashMap;
use std::collections::HashSet;
use tokio_postgres::tls::NoTlsStream;
use tokio_postgres::Socket;
use tokio_postgres::{Client, Connection, NoTls};

// Structure for storing key processing statistics
pub struct KeyInsertStats {
    pub total: usize,                   // Total number of received keys
    pub inserted: usize,                // Number of new keys
    pub unchanged: usize,               // Number of unchanged keys
    pub key_id_map: Vec<(SshKey, i32)>, // Mapping of keys to their IDs in the database
}

// Simple database client that exits on connection errors
pub struct DbClient {
    client: Client,
}

impl DbClient {
    pub async fn connect(
        connection_string: &str,
    ) -> Result<(Self, Connection<Socket, NoTlsStream>), tokio_postgres::Error> {
        info!("Connecting to database...");
        let (client, connection) = tokio_postgres::connect(connection_string, NoTls).await?;
        info!("Successfully connected to database");

        Ok((DbClient { client }, connection))
    }

    // Helper function to handle database errors - exits the application on connection errors
    fn handle_db_error<T>(
        result: Result<T, tokio_postgres::Error>,
        operation: &str,
    ) -> Result<T, tokio_postgres::Error> {
        match result {
            Ok(value) => Ok(value),
            Err(e) => {
                if Self::is_connection_error(&e) {
                    error!("Database connection lost during {}: {}", operation, e);
                    error!("Exiting application due to database connection failure");
                    std::process::exit(1);
                } else {
                    // For non-connection errors, just return the error
                    Err(e)
                }
            }
        }
    }

    fn is_connection_error(error: &tokio_postgres::Error) -> bool {
        // Check if the error is related to connection issues
        let error_str = error.to_string();
        error_str.contains("connection closed")
            || error_str.contains("connection reset")
            || error_str.contains("broken pipe")
            || error_str.contains("Connection refused")
            || error_str.contains("connection terminated")
            || error.as_db_error().is_none() // Non-database errors are often connection issues
    }

    pub async fn initialize_schema(&self) -> Result<(), tokio_postgres::Error> {
        info!("Checking and initializing database schema if needed");

        // Check if tables exist by querying information_schema
        let result = self
            .client
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
            .await;

        let tables_exist = Self::handle_db_error(result, "checking table existence")?
            .get(0)
            .map(|row| row.get::<_, bool>(0))
            .unwrap_or(false);

        if !tables_exist {
            info!("Database schema doesn't exist. Creating tables...");

            // Create the keys table
            let result = self
                .client
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
                .await;
            Self::handle_db_error(result, "creating keys table")?;

            // Create the flows table
            let result = self
                .client
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
                .await;
            Self::handle_db_error(result, "creating flows table")?;

            // Create an index for faster lookups
            let result = self
                .client
                .execute(
                    "CREATE INDEX IF NOT EXISTS idx_flows_name ON public.flows(name)",
                    &[],
                )
                .await;
            Self::handle_db_error(result, "creating index")?;

            info!("Database schema created successfully");
        } else {
            info!("Database schema already exists");

            // Check if deprecated column exists, add it if missing (migration)
            let result = self
                .client
                .query(
                    "SELECT EXISTS (
                        SELECT FROM information_schema.columns 
                        WHERE table_schema = 'public' 
                        AND table_name = 'keys' 
                        AND column_name = 'deprecated'
                    )",
                    &[],
                )
                .await;

            let column_exists = Self::handle_db_error(result, "checking deprecated column")?
                .get(0)
                .map(|row| row.get::<_, bool>(0))
                .unwrap_or(false);

            if !column_exists {
                info!("Adding deprecated column to existing keys table...");
                let result = self.client
                    .execute(
                        "ALTER TABLE public.keys ADD COLUMN deprecated BOOLEAN NOT NULL DEFAULT FALSE",
                        &[],
                    )
                    .await;
                Self::handle_db_error(result, "adding deprecated column")?;
                info!("Migration completed: deprecated column added");
            }
        }

        Ok(())
    }

    pub async fn batch_insert_keys(
        &self,
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
        let mut key_query =
            String::from("SELECT host, key, key_id, deprecated FROM public.keys WHERE ");

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

        let result = self.client.query(&key_query, &params[..]).await;
        let rows = Self::handle_db_error(result, "checking existing keys")?;

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
            let mut insert_sql =
                String::from("INSERT INTO public.keys (host, key, updated) VALUES ");

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

            let result = self.client.query(&insert_sql, &insert_params[..]).await;
            let inserted_rows = Self::handle_db_error(result, "inserting keys")?;

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
        &self,
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

        let result = self.client.query(&existing_query, &params[..]).await;
        let rows = Self::handle_db_error(result, "checking existing flow associations")?;

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
        let result = self.client.execute(&sql, &insert_params[..]).await;
        let affected = Self::handle_db_error(result, "inserting flow associations")?;

        let affected_usize = affected as usize;

        info!(
            "Added {} new key-flow associations for flow '{}' (skipped {} existing)",
            affected_usize,
            flow_name,
            existing_associations.len()
        );

        Ok(affected_usize)
    }

    pub async fn get_keys_from_db(
        &self,
    ) -> Result<Vec<crate::server::Flow>, tokio_postgres::Error> {
        let result = self.client.query(
            "SELECT k.host, k.key, k.deprecated, f.name FROM public.keys k INNER JOIN public.flows f ON k.key_id = f.key_id",
            &[]
        ).await;
        let rows = Self::handle_db_error(result, "getting keys from database")?;

        let mut flows_map: HashMap<String, crate::server::Flow> = HashMap::new();

        for row in rows {
            let host: String = row.get(0);
            let key: String = row.get(1);
            let deprecated: bool = row.get(2);
            let flow: String = row.get(3);

            let ssh_key = SshKey {
                server: host,
                public_key: key,
                deprecated,
            };

            if let Some(flow_entry) = flows_map.get_mut(&flow) {
                flow_entry.servers.push(ssh_key);
            } else {
                flows_map.insert(
                    flow.clone(),
                    crate::server::Flow {
                        name: flow,
                        servers: vec![ssh_key],
                    },
                );
            }
        }

        info!("Retrieved {} flows from database", flows_map.len());
        Ok(flows_map.into_values().collect())
    }

    pub async fn deprecate_key_by_server(
        &self,
        server_name: &str,
        flow_name: &str,
    ) -> Result<u64, tokio_postgres::Error> {
        // Update keys to deprecated status for the given server
        let result = self
            .client
            .execute(
                "UPDATE public.keys 
                 SET deprecated = TRUE, updated = NOW() 
                 WHERE host = $1 
                 AND key_id IN (
                     SELECT key_id FROM public.flows WHERE name = $2
                 )",
                &[&server_name, &flow_name],
            )
            .await;
        let affected = Self::handle_db_error(result, "deprecating key")?;

        info!(
            "Deprecated {} key(s) for server '{}' in flow '{}'",
            affected, server_name, flow_name
        );

        Ok(affected)
    }

    pub async fn restore_key_by_server(
        &self,
        server_name: &str,
        flow_name: &str,
    ) -> Result<u64, tokio_postgres::Error> {
        // Update keys to active status for the given server in the flow
        let result = self
            .client
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
            .await;
        let affected = Self::handle_db_error(result, "restoring key")?;

        info!(
            "Restored {} key(s) for server '{}' in flow '{}'",
            affected, server_name, flow_name
        );

        Ok(affected)
    }

    pub async fn permanently_delete_key_by_server(
        &self,
        server_name: &str,
        flow_name: &str,
    ) -> Result<u64, tokio_postgres::Error> {
        // First, find the key_ids for the given server in the flow
        let result = self
            .client
            .query(
                "SELECT k.key_id FROM public.keys k 
                 INNER JOIN public.flows f ON k.key_id = f.key_id 
                 WHERE k.host = $1 AND f.name = $2",
                &[&server_name, &flow_name],
            )
            .await;
        let key_rows = Self::handle_db_error(result, "finding keys to delete")?;

        if key_rows.is_empty() {
            return Ok(0);
        }

        let key_ids: Vec<i32> = key_rows.iter().map(|row| row.get::<_, i32>(0)).collect();

        // Delete flow associations first
        let mut flow_delete_count = 0;
        for key_id in &key_ids {
            let result = self
                .client
                .execute(
                    "DELETE FROM public.flows WHERE name = $1 AND key_id = $2",
                    &[&flow_name, key_id],
                )
                .await;
            let deleted = Self::handle_db_error(result, "deleting flow association")?;
            flow_delete_count += deleted;
        }

        // Check if any of these keys are used in other flows
        let mut keys_to_delete = Vec::new();
        for key_id in &key_ids {
            let result = self
                .client
                .query_one(
                    "SELECT COUNT(*) FROM public.flows WHERE key_id = $1",
                    &[key_id],
                )
                .await;
            let count: i64 = Self::handle_db_error(result, "checking key references")?.get(0);

            if count == 0 {
                keys_to_delete.push(*key_id);
            }
        }

        // Permanently delete keys that are no longer referenced by any flow
        let mut total_deleted = 0;
        for key_id in keys_to_delete {
            let result = self
                .client
                .execute("DELETE FROM public.keys WHERE key_id = $1", &[&key_id])
                .await;
            let deleted = Self::handle_db_error(result, "deleting key")?;
            total_deleted += deleted;
        }

        info!(
            "Permanently deleted {} flow associations and {} orphaned keys for server '{}' in flow '{}'",
            flow_delete_count, total_deleted, server_name, flow_name
        );

        Ok(std::cmp::max(flow_delete_count, total_deleted))
    }
}

// Compatibility wrapper for transition
pub struct ReconnectingDbClient {
    inner: Option<DbClient>,
}

impl ReconnectingDbClient {
    pub fn new(_connection_string: String) -> Self {
        Self { inner: None }
    }

    pub async fn connect(&mut self, connection_string: &str) -> Result<(), tokio_postgres::Error> {
        let (client, connection) = DbClient::connect(connection_string).await?;

        // Spawn connection handler that will exit on error
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                error!("Database connection error: {}", e);
                error!("Exiting application due to database connection failure");
                std::process::exit(1);
            }
        });

        self.inner = Some(client);
        Ok(())
    }

    pub async fn initialize_schema(&self) -> Result<(), tokio_postgres::Error> {
        match &self.inner {
            Some(client) => client.initialize_schema().await,
            None => panic!("Database client not initialized"),
        }
    }

    pub async fn batch_insert_keys_reconnecting(
        &self,
        keys: Vec<SshKey>,
    ) -> Result<KeyInsertStats, tokio_postgres::Error> {
        match &self.inner {
            Some(client) => client.batch_insert_keys(&keys).await,
            None => panic!("Database client not initialized"),
        }
    }

    pub async fn batch_insert_flow_keys_reconnecting(
        &self,
        flow_name: String,
        key_ids: Vec<i32>,
    ) -> Result<usize, tokio_postgres::Error> {
        match &self.inner {
            Some(client) => client.batch_insert_flow_keys(&flow_name, &key_ids).await,
            None => panic!("Database client not initialized"),
        }
    }

    pub async fn get_keys_from_db_reconnecting(
        &self,
    ) -> Result<Vec<crate::server::Flow>, tokio_postgres::Error> {
        match &self.inner {
            Some(client) => client.get_keys_from_db().await,
            None => panic!("Database client not initialized"),
        }
    }

    pub async fn deprecate_key_by_server_reconnecting(
        &self,
        server_name: String,
        flow_name: String,
    ) -> Result<u64, tokio_postgres::Error> {
        match &self.inner {
            Some(client) => {
                client
                    .deprecate_key_by_server(&server_name, &flow_name)
                    .await
            }
            None => panic!("Database client not initialized"),
        }
    }

    pub async fn restore_key_by_server_reconnecting(
        &self,
        server_name: String,
        flow_name: String,
    ) -> Result<u64, tokio_postgres::Error> {
        match &self.inner {
            Some(client) => client.restore_key_by_server(&server_name, &flow_name).await,
            None => panic!("Database client not initialized"),
        }
    }

    pub async fn permanently_delete_key_by_server_reconnecting(
        &self,
        server_name: String,
        flow_name: String,
    ) -> Result<u64, tokio_postgres::Error> {
        match &self.inner {
            Some(client) => {
                client
                    .permanently_delete_key_by_server(&server_name, &flow_name)
                    .await
            }
            None => panic!("Database client not initialized"),
        }
    }
}
