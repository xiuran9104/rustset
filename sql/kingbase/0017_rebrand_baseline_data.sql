-- Remove user-visible upstream demo branding from the RustSet baseline while
-- preserving technical compatibility identifiers used by existing records.

UPDATE system_tenant
SET name = CASE WHEN id = 1 AND name = '芋道源码' THEN 'RustSet' ELSE name END,
    contact_name = CASE
        WHEN contact_name IN ('芋艿', '芋道') THEN '管理员'
        ELSE contact_name
    END,
    websites = CASE
        WHEN websites ILIKE '%iocoder.cn%' OR websites ILIKE '%wxc4598c446f8a9cb3%'
            THEN ''
        ELSE websites
    END,
    updater = 'migration',
    update_time = now()
WHERE deleted = 0
  AND (
      (id = 1 AND name = '芋道源码')
      OR contact_name IN ('芋艿', '芋道')
      OR websites ILIKE '%iocoder.cn%'
      OR websites ILIKE '%wxc4598c446f8a9cb3%'
  );

UPDATE system_dept
SET name = 'RustSet', updater = 'migration', update_time = now()
WHERE deleted = 0 AND name = '芋道源码';

UPDATE system_users
SET nickname = CASE
        WHEN id = 1 AND nickname = '芋道源码' THEN 'RustSet 管理员'
        WHEN nickname IN ('芋道', '芋道1') THEN 'RustSet 用户'
        ELSE nickname
    END,
    remark = CASE WHEN remark = '不要吓我' THEN NULL ELSE remark END,
    updater = 'migration',
    update_time = now()
WHERE deleted = 0
  AND (
      (id = 1 AND nickname = '芋道源码')
      OR nickname IN ('芋道', '芋道1')
      OR remark = '不要吓我'
  );

UPDATE system_notice
SET deleted = 1, updater = 'migration', update_time = now()
WHERE deleted = 0
  AND tenant_id = 1
  AND (
      title = '芋道的公众'
      OR content ILIKE '%yudao.iocoder.cn%'
  );

UPDATE system_oauth2_client
SET name = 'RustSet',
    logo = '',
    description = 'RustSet 默认 OAuth2 客户端',
    redirect_uris = '["http://127.0.0.1:5666"]',
    updater = 'migration',
    update_time = now()
WHERE deleted = 0 AND client_id = 'default' AND name = '芋道源码';

UPDATE system_oauth2_client
SET deleted = 1, updater = 'migration', update_time = now()
WHERE deleted = 0
  AND (
      client_id LIKE 'yudao-%'
      OR logo ILIKE '%yudao.iocoder.cn%'
  )
  AND client_id <> 'default';
