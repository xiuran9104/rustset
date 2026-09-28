mod compat;
mod data_scope;
mod excel;
mod messaging;
mod notify;
mod shared;
mod tenant_csv;
mod user_relations;

use aide::axum::ApiRouter;

use crate::SystemState;

pub fn routes() -> ApiRouter<SystemState> {
    ApiRouter::new()
        .merge(compat::routes())
        .merge(tenant_csv::routes())
}
