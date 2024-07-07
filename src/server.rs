use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use log;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_postgres::{Client, NoTls};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SshKey {
    pub server: String,
    pub public_key: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Flow {
    pub name: String,
    pub servers: Vec<SshKey>,
}

pub type Flows = Arc<Mutex<Vec<Flow>>>;

pub fn is_valid_ssh_key(key: &str) -> bool {
    let rsa_re = Regex::new(r"^ssh-rsa AAAA[0-9A-Za-z+/]+[=]{0,3}( .+)?$").unwrap();
    let dsa_re = Regex::new(r"^ssh-dss AAAA[0-9A-Za-z+/]+[=]{0,3}( .+)?$").unwrap();
    let ecdsa_re =
        Regex::new(r"^ecdsa-sha2-nistp(256|384|521) AAAA[0-9A-Za-z+/]+[=]{0,3}( .+)?$").unwrap();
    let ed25519_re = Regex::new(r"^ssh-ed25519 AAAA[0-9A-Za-z+/]+[=]{0,3}( .+)?$").unwrap();

    rsa_re.is_match(key)
        || dsa_re.is_match(key)
        || ecdsa_re.is_match(key)
        || ed25519_re.is_match(key)
}

pub async fn insert_key_if_not_exists(
    client: &Client,
    key: &SshKey,
) -> Result<i32, tokio_postgres::Error> {
    let row = client
        .query_opt(
            "SELECT key_id FROM public.keys WHERE host = $1 AND key = $2",
            &[&key.server, &key.public_key],
        )
        .await?;

    if let Some(row) = row {
        client
            .execute(
                "UPDATE public.keys SET updated = NOW() WHERE key_id = $1",
                &[&row.get::<_, i32>(0)],
            )
            .await?;
        Ok(row.get(0))
    } else {
        let row = client.query_one(
            "INSERT INTO public.keys (host, key, updated) VALUES ($1, $2, NOW()) RETURNING key_id",
            &[&key.server, &key.public_key]
        ).await?;
        Ok(row.get(0))
    }
}

pub async fn insert_flow_key(
    client: &Client,
    flow_name: &str,
    key_id: i32,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO public.flows (name, key_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            &[&flow_name, &key_id],
        )
        .await?;
    Ok(())
}

pub async fn get_keys_from_db(client: &Client) -> Result<Vec<Flow>, tokio_postgres::Error> {
    let rows = client.query(
        "SELECT k.host, k.key, f.name FROM public.keys k INNER JOIN public.flows f ON k.key_id = f.key_id",
        &[]
    ).await?;

    let mut flows_map: HashMap<String, Flow> = HashMap::new();

    for row in rows {
        let host: String = row.get(0);
        let key: String = row.get(1);
        let flow: String = row.get(2);

        let ssh_key = SshKey {
            server: host,
            public_key: key,
        };

        if let Some(flow_entry) = flows_map.get_mut(&flow) {
            flow_entry.servers.push(ssh_key);
        } else {
            flows_map.insert(
                flow.clone(),
                Flow {
                    name: flow,
                    servers: vec![ssh_key],
                },
            );
        }
    }

    Ok(flows_map.into_values().collect())
}

pub async fn get_keys(
    flows: web::Data<Flows>,
    flow_id: web::Path<String>,
    allowed_flows: web::Data<Vec<String>>,
) -> impl Responder {
    let flow_id_str = flow_id.into_inner();
    if !allowed_flows.contains(&flow_id_str) {
        return HttpResponse::Forbidden().body("Flow ID not allowed");
    }

    let flows = flows.lock().unwrap();
    if let Some(flow) = flows.iter().find(|flow| flow.name == flow_id_str) {
        let servers: Vec<&SshKey> = flow.servers.iter().collect();
        HttpResponse::Ok().json(servers)
    } else {
        HttpResponse::NotFound().body("Flow ID not found")
    }
}

pub async fn add_keys(
    flows: web::Data<Flows>,
    flow_id: web::Path<String>,
    new_keys: web::Json<Vec<SshKey>>,
    db_client: web::Data<Arc<Client>>,
    allowed_flows: web::Data<Vec<String>>,
) -> impl Responder {
    let flow_id_str = flow_id.into_inner();
    if !allowed_flows.contains(&flow_id_str) {
        return HttpResponse::Forbidden().body("Flow ID not allowed");
    }

    for new_key in new_keys.iter() {
        if !is_valid_ssh_key(&new_key.public_key) {
            return HttpResponse::BadRequest().body(format!(
                "Invalid SSH key format for server: {}",
                new_key.server
            ));
        }

        match insert_key_if_not_exists(&db_client, new_key).await {
            Ok(key_id) => {
                if let Err(e) = insert_flow_key(&db_client, &flow_id_str, key_id).await {
                    log::error!("Failed to insert flow key into database: {}", e);
                    return HttpResponse::InternalServerError()
                        .body("Failed to insert flow key into database");
                }
            }
            Err(e) => {
                log::error!("Failed to insert key into database: {}", e);
                return HttpResponse::InternalServerError()
                    .body("Failed to insert key into database");
            }
        }
    }

    // Refresh the flows data from the database
    let updated_flows = get_keys_from_db(&db_client)
        .await
        .unwrap_or_else(|_| Vec::new());
    let mut flows_guard = flows.lock().unwrap();
    *flows_guard = updated_flows;

    let updated_flow = flows_guard.iter().find(|flow| flow.name == flow_id_str);
    if let Some(flow) = updated_flow {
        let servers: Vec<&SshKey> = flow.servers.iter().collect();
        HttpResponse::Ok().json(servers)
    } else {
        HttpResponse::NotFound().body("Flow ID not found")
    }
}

pub async fn run_server(args: crate::Args) -> std::io::Result<()> {
    let db_user = args.db_user.expect("db_user is required in server mode");
    let db_password = args
        .db_password
        .expect("db_password is required in server mode");

    let db_conn_str = format!(
        "host={} user={} password={} dbname={}",
        args.db_host, db_user, db_password, args.db_name
    );

    let (db_client, connection) = tokio_postgres::connect(&db_conn_str, NoTls).await.unwrap();
    let db_client = Arc::new(db_client);

    // Spawn a new thread to run the database connection
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    let mut initial_flows = get_keys_from_db(&db_client)
        .await
        .unwrap_or_else(|_| Vec::new());

    // Ensure all allowed flows are initialized
    for allowed_flow in &args.flows {
        if !initial_flows.iter().any(|flow| &flow.name == allowed_flow) {
            initial_flows.push(Flow {
                name: allowed_flow.clone(),
                servers: vec![],
            });
        }
    }

    let flows: Flows = Arc::new(Mutex::new(initial_flows));
    let allowed_flows = web::Data::new(args.flows);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(flows.clone()))
            .app_data(web::Data::new(db_client.clone()))
            .app_data(allowed_flows.clone())
            .route("/{flow_id}/keys", web::get().to(get_keys))
            .route("/{flow_id}/keys", web::post().to(add_keys))
    })
    .bind((args.ip.as_str(), args.port))?
    .run()
    .await
}
