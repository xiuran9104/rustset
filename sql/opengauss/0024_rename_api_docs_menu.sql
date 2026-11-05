-- 0024: the API documentation page renders with Scalar, so the legacy
-- 'swagger' URL segment and component name no longer describe it. Rename
-- to api-docs; the frontend keeps a redirect for old bookmarks.

UPDATE system_menu
SET path = 'api-docs',
    component = 'infra/api-docs/index',
    component_name = 'InfraApiDocs',
    updater = 'migration',
    update_time = now()
WHERE id = 116
  AND deleted = 0
  AND component = 'infra/swagger/index';

UPDATE system_menu
SET path = 'api-docs',
    updater = 'migration',
    update_time = now()
WHERE id = 116
  AND deleted = 0
  AND path = 'swagger';
