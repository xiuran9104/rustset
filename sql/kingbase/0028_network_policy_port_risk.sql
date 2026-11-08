-- Match tenant-owned allow policies against an auditable high-risk port baseline.
CREATE SEQUENCE IF NOT EXISTS public.infra_high_risk_port_rule_seq
    START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

CREATE TABLE IF NOT EXISTS public.infra_high_risk_port_rule (
    id bigint DEFAULT nextval('public.infra_high_risk_port_rule_seq'::regclass) NOT NULL,
    name character varying(128) NOT NULL,
    protocol character varying(16) DEFAULT 'tcp'::character varying NOT NULL,
    port_start integer NOT NULL,
    port_end integer NOT NULL,
    severity character varying(32) NOT NULL,
    description text NOT NULL,
    solution text NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    creator character varying(64) DEFAULT ''::character varying NOT NULL,
    create_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updater character varying(64) DEFAULT ''::character varying NOT NULL,
    update_time timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted smallint DEFAULT 0 NOT NULL,
    CONSTRAINT infra_high_risk_port_rule_pkey PRIMARY KEY (id),
    CONSTRAINT infra_high_risk_port_rule_protocol_check CHECK (protocol IN ('tcp', 'udp', 'any')),
    CONSTRAINT infra_high_risk_port_rule_range_check CHECK (
        port_start BETWEEN 1 AND 65535 AND port_end BETWEEN port_start AND 65535
    ),
    CONSTRAINT infra_high_risk_port_rule_severity_check CHECK (
        severity IN ('Critical', 'High', 'Medium', 'Low')
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_high_risk_port_rule_identity
    ON public.infra_high_risk_port_rule(protocol, port_start, port_end)
    WHERE deleted = 0;

INSERT INTO public.infra_high_risk_port_rule
    (name, protocol, port_start, port_end, severity, description, solution)
VALUES
    ('FTP 明文服务', 'tcp', 21, 21, 'High', 'FTP 使用明文认证与传输，暴露后易造成凭据和数据泄露。', '关闭公网访问；确需使用时限制可信源地址并迁移到 SFTP。'),
    ('SSH 远程管理', 'tcp', 22, 22, 'High', '远程管理端口暴露会增加口令爆破和未授权访问风险。', '限制到运维网或堡垒机，启用密钥认证和多因素认证。'),
    ('Telnet 明文管理', 'tcp', 23, 23, 'Critical', 'Telnet 以明文传输认证信息，不应跨不可信网络开放。', '立即关闭 Telnet 并迁移到 SSH，通过管理网或堡垒机访问。'),
    ('Windows RPC', 'tcp', 135, 135, 'High', 'RPC 端口暴露可能泄露主机信息并扩大远程攻击面。', '限制到必要的管理网段，并通过主机和边界防火墙双重控制。'),
    ('NetBIOS', 'any', 137, 139, 'High', 'NetBIOS 服务暴露会泄露名称与共享信息，并增加横向移动风险。', '关闭跨安全域访问；无法关闭时仅允许受控内网源地址。'),
    ('SMB 文件共享', 'tcp', 445, 445, 'Critical', 'SMB 暴露存在勒索软件传播、凭据窃取和远程利用风险。', '禁止公网和非必要跨域访问，限制可信源并及时修补系统。'),
    ('Microsoft SQL Server', 'tcp', 1433, 1433, 'High', '数据库端口暴露会增加爆破、数据泄露和漏洞利用风险。', '仅允许应用服务器网段访问，禁止用户网与公网直连。'),
    ('Oracle Database', 'tcp', 1521, 1521, 'High', '数据库监听端口暴露会增加未授权访问和数据泄露风险。', '仅允许指定应用源地址访问，并启用数据库访问审计。'),
    ('NFS 文件共享', 'any', 2049, 2049, 'High', 'NFS 暴露可能导致未授权挂载和敏感文件泄露。', '限制到必要的受控主机，核查导出权限并禁用匿名映射。'),
    ('Docker Remote API', 'tcp', 2375, 2375, 'Critical', '未加密 Docker API 可能允许直接控制主机和容器。', '关闭 2375；如需远程管理，使用双向 TLS 的 2376 并限制源地址。'),
    ('MySQL', 'tcp', 3306, 3306, 'High', '数据库端口暴露会增加爆破、数据泄露和漏洞利用风险。', '仅允许指定应用网段访问，使用最小权限账号并启用审计。'),
    ('RDP 远程桌面', 'tcp', 3389, 3389, 'Critical', '远程桌面暴露容易遭受口令爆破、凭据攻击和远程利用。', '仅通过 VPN 或堡垒机访问，限制源地址并启用网络级认证和多因素认证。'),
    ('PostgreSQL', 'tcp', 5432, 5432, 'High', '数据库端口暴露会增加未授权访问和数据泄露风险。', '仅允许指定应用服务器访问，并核查 pg_hba.conf 与账号权限。'),
    ('VNC 远程控制', 'tcp', 5900, 5900, 'Critical', 'VNC 暴露可能导致弱口令攻击、会话窃听和远程控制。', '关闭直接暴露，通过堡垒机或 VPN 访问并启用强认证。'),
    ('Redis', 'tcp', 6379, 6379, 'Critical', 'Redis 暴露可能导致未授权读写、数据泄露或主机命令执行。', '绑定内网地址，启用认证和 ACL，仅允许必要应用源地址访问。'),
    ('Elasticsearch', 'tcp', 9200, 9200, 'High', 'Elasticsearch 接口暴露可能导致索引数据泄露和未授权操作。', '启用认证与 TLS，仅允许受控应用和运维网段访问。'),
    ('Memcached', 'any', 11211, 11211, 'Critical', 'Memcached 暴露可能泄露缓存数据，并被用于反射放大攻击。', '绑定内网地址，关闭 UDP，并限制为必要应用源地址。'),
    ('MongoDB', 'tcp', 27017, 27017, 'Critical', 'MongoDB 暴露可能导致未授权访问和业务数据泄露。', '启用认证与 TLS，仅允许指定应用服务器访问。')
ON CONFLICT DO NOTHING;

ALTER TABLE public.infra_risk
    ADD COLUMN IF NOT EXISTS source_type character varying(64),
    ADD COLUMN IF NOT EXISTS source_id bigint,
    ADD COLUMN IF NOT EXISTS rule_id bigint;

-- A network policy may contain a CIDR or an address list, so the risk subject
-- must be able to retain the complete destination expression.
ALTER TABLE public.infra_risk
    ALTER COLUMN asset_ip TYPE character varying(256);

CREATE INDEX IF NOT EXISTS idx_infra_risk_policy_source
    ON public.infra_risk(tenant_id, source_type, source_id)
    WHERE deleted = 0;

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.infra_risk'::regclass
          AND conname = 'infra_risk_high_risk_rule_fk'
    ) THEN
        ALTER TABLE public.infra_risk
            ADD CONSTRAINT infra_risk_high_risk_rule_fk
            FOREIGN KEY (rule_id) REFERENCES public.infra_high_risk_port_rule(id);
    END IF;
END $$;
