use std::collections::HashSet;
use anyhow::{Context, Result};
use chrono::{NaiveDate, NaiveDateTime, Utc};
use log::{debug, info, trace, LevelFilter};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use std::io::Write;
use clap::value_parser;

#[derive(Debug, Serialize, Deserialize)]
struct Incident {
    #[serde(rename = "incidentID")]
    incident_id: i32,
    #[serde(rename = "orgPublishDate")]
    org_publish_date: NaiveDate,
    #[serde(deserialize_with = "parse_naive_datetime")]
    #[serde(rename = "modifiedDate")]
    modified_date: NaiveDateTime,
    published: i32,
    country: String,
    #[serde(rename = "incidentText")]
    incident_text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct IncidentDetail {
    #[serde(rename = "publishDate")]
    publish_date: NaiveDate,
    #[serde(rename = "affectedObj")]
    affected_obj: String,
    #[serde(rename = "affectedType")]
    affected_type: String,
    #[serde(rename = "description_de")]
    details_text: String,
    tags: String,
    href: String,
    reference: String,
}

fn parse_naive_datetime<'de, D>(deserializer: D) -> Result<NaiveDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| serde::de::Error::custom(format!("Failed to parse datetime '{}': {}", s, e)))
}

fn setup_logger() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            let timestamp = Utc::now().to_rfc3339();
            writeln!(
                buf,
                "{} [{}] {}: {}",
                timestamp,
                record.target(),
                record.level(),
                record.args()
            )
        })
        .filter_module("dsgvo_downloader", LevelFilter::Trace)
        .init();
}

async fn setup_database(database_url: &str) -> Result<sqlx::PgPool> {
    trace!("Setting up database");
    debug!("Using database url: {}", database_url);

    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Failed to connect to database")
}

async fn verify_tables(pool: &sqlx::PgPool) -> Result<()> {
    trace!("Verifying tables in database");
    let tables: Vec<String> = sqlx::query_scalar(
        r#"SELECT table_name FROM information_schema.tables
           WHERE table_schema = 'public'
           AND table_name IN ('incidents', 'incident_history')"#,
    )
        .fetch_all(pool)
        .await
        .context("Failed to verify tables")?;

    debug!("Found {} tables in database: {:?}, expected to be present: incidents & incident_history", tables.len(), tables);

    if tables.len() != 2 {
        anyhow::bail!("Missing required database tables");
    }
    Ok(())
}

/// Fetch existing incident ids from the website
async fn get_existing_incident_ids(pool: &sqlx::PgPool) -> Result<HashSet<i32>> {
    trace!("Getting existing incident ids from database");
    let ids: Vec<i32> = sqlx::query_scalar("SELECT incident_id FROM incidents")
        .fetch_all(pool)
        .await
        .context("Failed to fetch existing incident IDs")?;
    trace!("Found existing incident ids: {:?}", ids);
    Ok(ids.into_iter().collect())
}

/// Fetch incidents from the website
async fn fetch_incidents(pool: &sqlx::PgPool) -> Result<Vec<Incident>> {
    info!("Fetching incidents from website");
    let client = reqwest::Client::new();
    let response = client
        .get("https://www.dsgvo-portal.de/sicherheitsvorfall-datenbank/?cmd=getIncidents")
        .header("Accept", "application/json")
        .header("Referer", "https://www.dsgvo-portal.de/sicherheitsvorfall-datenbank/")
        .send()
        .await
        .context("Failed to fetch incidents")?;
    trace!("Got cmd response: {}, getting body", response.status());
    let body = response.text().await.context("Failed to read response body")?;
    trace!("Successfully got body");

    let trimmed = body.trim();

    trace!("Storing raw response");
    // Store raw response before parsing
    store_raw_response(pool, trimmed).await?;

    serde_json::from_str(trimmed)
        .context("Failed to parse incident response")
}

async fn store_raw_response(pool: &sqlx::PgPool, content: &str) -> Result<()> {
    trace!("Storing raw incident history");
    sqlx::query("INSERT INTO incident_history (content) VALUES ($1::jsonb)")
        .bind(content)
        .execute(pool)
        .await
        .context("Failed to store raw response")?;
    Ok(())
}

async fn process_new_incidents(incidents: Vec<Incident>, pool: &sqlx::PgPool, request_delay: u64) -> Result<()> {
    trace!("Processing {} new incidents: {:?}", incidents.len(), incidents);
    let client = reqwest::Client::new();

    for incident in incidents {
        let id = incident.incident_id;
        debug!("Processing incident: {}", id);
        process_incident(&client, &pool, incident)
            .await
            .context(format!("Failed to process incident: {}", id))?;
        tokio::time::sleep(Duration::from_millis(request_delay)).await;
    }

    Ok(())
}

async fn process_incident(client: &reqwest::Client, pool: &sqlx::PgPool, incident: Incident) -> Result<()> {
    debug!("Processing incident {}", incident.incident_id);
    let detail = fetch_incident_detail(client, incident.incident_id).await?;
    store_incident(pool, &incident, &detail).await?;
    Ok(())
}

async fn fetch_incident_detail(client: &reqwest::Client, incident_id: i32) -> Result<IncidentDetail> {
    debug!("Fetching incident detail from website for incident {}", incident_id);
    let url = format!(
        "https://www.dsgvo-portal.de/sicherheitsvorfall-datenbank/incidentDetails.php?incident={}",
        incident_id
    );
    trace!("Fetching url: {}", url);

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .header("Referer", "https://www.dsgvo-portal.de/sicherheitsvorfaelle/")
        .send()
        .await
        .with_context(|| format!("Failed to fetch details for incident {}", incident_id))?;

    trace!("Response status: {}", response.status());

    if !response.status().is_success() {
        anyhow::bail!("Unexpected status code: {}", response.status());
    }

    let body = response.text().await
        .with_context(|| format!("Failed to read response body for incident {}", incident_id))?;

    trace!("Response body: {}", body.trim());

    serde_json::from_str(body.trim())
        .with_context(|| format!("Failed to parse details for incident {}", incident_id))
}

async fn store_incident(pool: &sqlx::PgPool, incident: &Incident, detail: &IncidentDetail) -> Result<()> {
    trace!("Storing incident: {}", incident.incident_id);

    let parsed: serde_json::Value = serde_json::from_str(&detail.reference).context("Failed to parse references in details")?;

    sqlx::query(
        r#"INSERT INTO incidents (
            incident_id, org_publish_date, modified_date, published, publish_date,
            affected_obj, affected_type, country, details_text, tags, href,
            "references", incident_text
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::jsonb, $13)"#,
    )
        .bind(incident.incident_id)
        .bind(incident.org_publish_date)
        .bind(incident.modified_date.clone())
        .bind(incident.published)
        .bind(detail.publish_date.clone())
        .bind(&detail.affected_obj)
        .bind(&detail.affected_type)
        .bind(&incident.country)
        .bind(&detail.details_text)
        .bind(&detail.tags)
        .bind(&detail.href)
        .bind(&parsed)
        .bind(&incident.incident_text)
        .execute(pool)
        .await
        .with_context(|| format!("Failed to store incident {}", incident.incident_id))?;

    info!("Successfully stored incident {}", incident.incident_id);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    setup_logger();

    let matches = clap::builder::Command::new("dsgvo-downloader")
        .arg(clap::Arg::new("delay")
            .short('d')
            .long("delay")
            .default_value("500")
            .action(clap::ArgAction::Set)
            .value_parser(value_parser!(u64))
            .help("Delay time in milliseconds")
            .long_help("Delay time in milliseconds as to not overwhelm the server and disable the api")
        )
        .arg(clap::Arg::new("database-url")
            .short('u')
            .long("database-url")
            .default_value("postgres://postgres@localhost:5432/dsgvo")
            .action(clap::ArgAction::Set)
            .value_parser(value_parser!(String))
            .help("Database URL for a postgres instance")
            .long_help("Database URL for a postgres instance, the tables have to be preconfigured via `schema.sql`")
        )
        .get_matches();

    let delay: u64 = *matches.get_one("delay").context("missing required argument delay")?;
    if delay < 500 {
        log::error!("delay has a minimum of 500ms");
    }

    let database_url: &str = matches.get_one("database-url").context("missing required argument database-url").map(String::as_str)?;

    trace!("Setting up database pool and verifying tables");
    let pool = setup_database(database_url).await?;
    verify_tables(&pool).await?;

    trace!("Fetching existing incidents");
    let existing_ids = get_existing_incident_ids(&pool).await?;
    trace!("Fetching incidents from website");
    let current_incidents = fetch_incidents(&pool).await?;

    // Filter for new incidents
    let new_incidents: Vec<_> = current_incidents
        .into_iter()
        .filter(|incident| !existing_ids.contains(&incident.incident_id))
        .collect();

    info!("Found {} new incidents", new_incidents.len());
    process_new_incidents(new_incidents, &pool, delay).await?;

    Ok(())
}
