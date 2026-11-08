-- Explicit approved TCP baselines are separate from observed asset ports.
CREATE TABLE IF NOT EXISTS infra_inspection_baseline (
    ip inet PRIMARY KEY,
    allowed_ports integer[] NOT NULL,
    reason text NOT NULL,
    updated_by varchar(64) NOT NULL,
    update_time timestamp NOT NULL DEFAULT now(),
    CHECK (0 < ALL(allowed_ports) AND 65536 > ALL(allowed_ports))
);
ALTER TABLE infra_task
    ADD COLUMN IF NOT EXISTS task_kind varchar(32) NOT NULL DEFAULT 'scan',
    ADD COLUMN IF NOT EXISTS scan_ports integer[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS total_targets integer NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS completed_targets integer NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS error_message text;
CREATE TABLE IF NOT EXISTS infra_inspection_result (
    id varchar(64) PRIMARY KEY,
    task_id varchar(64) NOT NULL,
    ip inet NOT NULL,
    registered boolean NOT NULL,
    baseline_ports integer[],
    open_ports integer[] NOT NULL,
    uncertain_ports integer[] NOT NULL,
    differences jsonb NOT NULL,
    risk_ids text[] NOT NULL DEFAULT '{}',
    create_time timestamp NOT NULL DEFAULT now(),
    UNIQUE(task_id, ip)
);
CREATE INDEX IF NOT EXISTS idx_inspection_result_task ON infra_inspection_result(task_id);
ALTER TABLE infra_risk ADD COLUMN IF NOT EXISTS inspection_key text;
CREATE UNIQUE INDEX IF NOT EXISTS idx_risk_inspection_key
    ON infra_risk(inspection_key) WHERE deleted=0 AND inspection_key IS NOT NULL;

-- 0020 inserted the physical-resource menu with an explicit id, so the
-- sequence can lag behind MAX(id); realign before allocating new menu ids.
SELECT setval(
    'system_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_menu),
        (SELECT last_value FROM system_menu_seq)
    ),
    true
);
INSERT INTO system_menu
    (id,name,permission,type,sort,parent_id,path,icon,component,component_name,
     status,visible,keep_alive,always_show,creator,updater,deleted)
SELECT nextval('system_menu_seq'),'资产核查','',2,5,id,'inspection','lucide:scan-search',
       'asset-ops/inspection/index','AssetInspection',0,true,true,true,'migration','migration',0
FROM system_menu d WHERE d.path='/asset-center' AND d.deleted=0
AND NOT EXISTS (SELECT 1 FROM system_menu WHERE component='asset-ops/inspection/index' AND deleted=0);

-- Reuse the existing scan execution, asset maintenance and risk review permissions.
SELECT setval(
    'system_role_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_role_menu),
        (SELECT last_value FROM system_role_menu_seq)
    ),
    true
);
INSERT INTO system_role_menu(id,role_id,menu_id,creator,updater,deleted,tenant_id)
SELECT nextval('system_role_menu_seq'), g.role_id, page.id,'migration','migration',0,g.tenant_id
FROM system_menu old_page
JOIN system_role_menu g ON g.menu_id=old_page.id AND g.deleted=0
CROSS JOIN system_menu page
WHERE old_page.component='asset-ops/task/index' AND old_page.deleted=0
AND page.component='asset-ops/inspection/index' AND page.deleted=0
AND NOT EXISTS (SELECT 1 FROM system_role_menu e WHERE e.role_id=g.role_id AND e.menu_id=page.id AND e.deleted=0);
