use anyhow::Context;
use rustset_framework_database::DbTransaction;

pub async fn replace_posts(
    transaction: &mut DbTransaction<'_>,
    user_id: i64,
    post_ids: &[i64],
    operator: &str,
    tenant_id: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE system_user_post
         SET deleted = 1, updater = $2, update_time = now()
         WHERE user_id = $1 AND deleted = 0",
    )
    .bind(user_id)
    .bind(operator)
    .execute(&mut **transaction)
    .await
    .context("failed to clear user posts")?;

    for post_id in post_ids {
        sqlx::query(
            "INSERT INTO system_user_post
             (id, user_id, post_id, creator, create_time, updater, update_time, deleted, tenant_id)
             SELECT nextval('system_user_post_seq'), $1, post.id, $3, now(), $3, now(), 0, $4
             FROM system_post post
             WHERE post.id = $2 AND post.deleted = 0 AND post.tenant_id = $4",
        )
        .bind(user_id)
        .bind(post_id)
        .bind(operator)
        .bind(tenant_id)
        .execute(&mut **transaction)
        .await
        .context("failed to assign user post")?
        .rows_affected()
        .eq(&1)
        .then_some(())
        .context("post does not exist in the user's tenant")?;
    }
    Ok(())
}

pub async fn replace_roles(
    transaction: &mut DbTransaction<'_>,
    user_id: i64,
    role_ids: &[i64],
    operator: &str,
    tenant_id: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE system_user_role
         SET deleted = 1, updater = $2, update_time = now()
         WHERE user_id = $1 AND deleted = 0",
    )
    .bind(user_id)
    .bind(operator)
    .execute(&mut **transaction)
    .await
    .context("failed to clear user roles")?;

    for role_id in role_ids {
        sqlx::query(
            "INSERT INTO system_user_role
             (id, user_id, role_id, creator, create_time, updater, update_time, deleted, tenant_id)
             SELECT nextval('system_user_role_seq'), $1, role.id, $3, now(), $3, now(), 0, $4
             FROM system_role role
             WHERE role.id = $2 AND role.deleted = 0 AND role.tenant_id = $4",
        )
        .bind(user_id)
        .bind(role_id)
        .bind(operator)
        .bind(tenant_id)
        .execute(&mut **transaction)
        .await
        .context("failed to assign user role")?
        .rows_affected()
        .eq(&1)
        .then_some(())
        .context("role does not exist in the user's tenant")?;
    }
    Ok(())
}

pub async fn delete_all(
    transaction: &mut DbTransaction<'_>,
    user_id: i64,
    operator: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE system_user_post SET deleted = 1, updater = $2, update_time = now()
         WHERE user_id = $1 AND deleted = 0",
    )
    .bind(user_id)
    .bind(operator)
    .execute(&mut **transaction)
    .await
    .context("failed to delete user posts")?;
    sqlx::query(
        "UPDATE system_user_role SET deleted = 1, updater = $2, update_time = now()
         WHERE user_id = $1 AND deleted = 0",
    )
    .bind(user_id)
    .bind(operator)
    .execute(&mut **transaction)
    .await
    .context("failed to delete user roles")?;
    Ok(())
}
