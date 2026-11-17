use rustset_framework_security::{CurrentUser, Permission};
use rustset_framework_web::AppError;
use uuid::Uuid;

pub fn require(user: &CurrentUser, code: &str) -> Result<(), AppError> {
    if user.role_codes.iter().any(|role| role == "super_admin") {
        return Ok(());
    }
    let permission = Permission::new(code).map_err(|_| AppError::internal("invalid policy"))?;
    if user.can(&permission) {
        Ok(())
    } else {
        Err(AppError::forbidden("permission denied"))
    }
}

pub fn parse_id(id: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(id).map_err(|_| AppError::bad_request("invalid id"))
}
