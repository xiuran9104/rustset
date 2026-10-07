use aide::openapi::{Info, OpenApi};
use rustset_infra_server::{InfraState, object_storage::ObjectStorage};
use sqlx::postgres::PgPoolOptions;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new().connect_lazy("postgres://openapi:openapi@127.0.0.1/openapi")?;
    let storage = ObjectStorage::from_env().map_err(std::io::Error::other)?;
    let mut document = OpenApi {
        info: Info {
            title: "RustSet Infra API".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            ..Info::default()
        },
        ..OpenApi::default()
    };
    let _router =
        rustset_infra_server::routes(InfraState::new(pool, storage)).finish_api(&mut document);
    serde_json::to_writer_pretty(std::io::stdout(), &document)?;
    Ok(())
}
