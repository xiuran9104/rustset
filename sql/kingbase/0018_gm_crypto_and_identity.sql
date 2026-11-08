-- 0018: 国密 internals rollout — SM3 identity UUIDs, SM3 password hash for the
-- seeded admin, SM4 re-seal of legacy XOR-seeded secrets, plus the last
-- user-visible upstream naming residue. Idempotent: every UPDATE is keyed on
-- the exact pre-change value.

-- 1) Per-user identity UUID derived from SM3("rustset-user:" || id) in Rust.
--    SQL cannot compute SM3, so baseline users carry literals here; the
--    gateway startup pass backfills any user created outside the baseline.
ALTER TABLE system_users ADD COLUMN IF NOT EXISTS identity_uuid uuid;

CREATE UNIQUE INDEX IF NOT EXISTS system_users_identity_uuid_key
    ON system_users (identity_uuid);

UPDATE system_users SET identity_uuid = buried.uuid
FROM (VALUES
    (1,    '4003435d-fee2-433d-8288-8bd84101bf16'::uuid),
    (100,  '0ba69165-1297-4983-a7e3-bbc36cf3b3a5'),
    (103,  'b20b44a9-e18e-4a7b-824d-e50b779fa649'),
    (104,  '2d10acdc-a623-4526-822f-295b76331f61'),
    (107,  'dfdc2f63-0643-4bf7-996e-d66b197ade02'),
    (108,  '34001a93-ad57-4df9-b797-86810bc7c88d'),
    (109,  '3e44df26-102a-4bb6-8b4c-1c06acee548b'),
    (110,  '9c330aef-242c-4349-8f10-e9f8a4f27a4d'),
    (111,  'd81ad3bf-9a1f-4420-97f1-11e17779468c'),
    (112,  '56c8c8f0-832e-4bcf-a4de-cb6d3f9b317f'),
    (113,  'f762824c-863d-4d6a-9652-d67d07dff70d'),
    (114,  'd1bdf970-446b-4dc2-abf4-2f4c969d2298'),
    (115,  '23c9ef7b-1947-4f3a-a06e-71488de6b507'),
    (117,  '783c8126-ed7b-4f47-b01c-74bfc7358601'),
    (118,  '1337338d-a3ec-4b36-9489-61bc4deb8c8a'),
    (139,  '51591a2c-4497-46d5-9158-f9d3a61220d6'),
    (141,  '445f0226-93a6-44ab-9553-61304601f847'),
    (142,  'a11d3ba6-9ddc-4914-816e-1d1b0f63423e'),
    (143,  '4f0cadfd-bd5e-4ed2-aa93-92eff0caff71'),
    (144,  'a4edfe32-96f1-4a81-84d3-59553729bb02')
) AS buried(id, uuid)
WHERE system_users.id = buried.id AND system_users.identity_uuid IS NULL;

-- 2) Replace the seeded admin's bcrypt hash with PBKDF2-HMAC-SM3("admin123").
--    Only the untouched baseline hash is replaced; changed passwords survive.
--    The $sm3$ encoding (108 chars) needs more room than bcrypt's 60.
ALTER TABLE system_users ALTER COLUMN password TYPE varchar(128);

UPDATE system_users
SET password = '$sm3$16384$7a7092f2a3f9a71facbcb3a1666a9110$f5e509bec314404aa6f771a1378b9e6645f386b88c692675267f89789ebc5010',
    updater = 'migration', update_time = now()
WHERE id = 1
  AND password = '$2a$04$.vd8nPeLwxt6hnSzmAoAyul8BOLX7Cib6QhcxRe30rfvrIPQHH1OG';

-- 3) Re-seal seeded secrets from the retired XOR enc:v1: format to SM4-CBC
--    (same default sealing key the baseline values carried).
UPDATE system_mail_account SET password = 'enc:sm4:v2:Tpnlc4jTjeUhzNQAfR+K1Zx2Qbfvb1eRClwXThjK1MBZ4sMnQZS67Y1yz8gClUKP'
WHERE id = 1 AND password = 'enc:v1:bm90LWNvbmZpZ3VyZWQ=';
UPDATE system_mail_account SET password = 'enc:sm4:v2:Y7MQnUONNTSjw7iSaiLxhrIf62J6VhCNv36lRl3BYfGEQXal2+iXu7zBBLojWT5z'
WHERE id = 2 AND password = 'enc:v1:bm90LWNvbmZpZ3VyZWQ=';
UPDATE system_mail_account SET password = 'enc:sm4:v2:FAqts/i4oybhm3hAxxAVocK9BPv/qPatpz72HIrei5HGKsA7/GHGDWu42L+sASq+'
WHERE id = 3 AND password = 'enc:v1:bm90LWNvbmZpZ3VyZWQ=';
UPDATE system_mail_account SET password = 'enc:sm4:v2:05FpuNGPizJXwGCjfa20HHRbnrquGzn/llMwHDwQOjFshz7EwINkJUKZLjteyy68'
WHERE id = 4 AND password = 'enc:v1:bm90LWNvbmZpZ3VyZWQ=';

UPDATE system_sms_channel
SET api_key = 'enc:sm4:v2:FE8ercajBVifj3mqLWipFEwG/M98RA0UrKbcYJzCQIjAjX+04uno/Z3IFKvF3cFP',
    api_secret = 'enc:sm4:v2:l84j7zWPF+756+aGEXMjzsVR1joEPz0o7EKppOD9XT/98Wch2DamzdY9n+jWlUp/'
WHERE id = 2 AND api_key = 'enc:v1:bm90LWNvbmZpZ3VyZWQ=';
UPDATE system_sms_channel
SET api_key = 'enc:sm4:v2:kwqYsxwMfw7nILox9EHbllCigUa7/ZSEL65JZBmJ30I3MUHHwEMZp/pQ1m+IQ6o5',
    api_secret = 'enc:sm4:v2:daOafQTreIJjHo/OJmlZuzbjEI9bi2C8b3pHrJf0LoQGsbymlk6p5akkDxkhmhoc'
WHERE id = 4 AND api_key = 'enc:v1:bm90LWNvbmZpZ3VyZWQ=';
UPDATE system_sms_channel
SET api_key = 'enc:sm4:v2:B+x5vhvQiPG6bcIoNXCdoUudOsBL9W4vhY7DkspZ5cJbmKaipEzcY0HRWb9i5T9x',
    api_secret = 'enc:sm4:v2:gKzSJ+izf0xglm2KxGO6FXtCAXWskw+gkNo4v8G98Oc0QxPL94oONlqUBsfbNK7T'
WHERE id = 7 AND api_key = 'enc:v1:bm90LWNvbmZpZ3VyZWQ=';

UPDATE infra_data_source_config SET password = 'enc:sm4:v2:PcfyYNzKJb/N5U/tCQt4feabqbQK0yxYGdINBJDaAWCLTtgWTb7kkTY+7uEoF/11'
WHERE id = 1 AND password = 'enc:v1:QA==';
UPDATE infra_data_source_config SET password = 'enc:sm4:v2:t12zrkO0dDzCTRWAWB317thkd5P/51APYmnJcon+xOs/5pOA6aokJFLcOl2BTYFb'
WHERE id = 2 AND password = 'enc:v1:QA==';

-- 4) Last user-visible upstream naming residue (0017 covered the bulk).
UPDATE system_users SET nickname = 'RustSet 用户', updater = 'migration', update_time = now()
WHERE deleted = 0 AND nickname = '芋艿';
UPDATE system_users SET username = 'rustset-demo', updater = 'migration', update_time = now()
WHERE deleted = 0 AND username = 'yudao';
UPDATE system_users SET nickname = '示例用户', updater = 'migration', update_time = now()
WHERE deleted = 0 AND nickname = '源码';
UPDATE system_users SET email = '', updater = 'migration', update_time = now()
WHERE deleted = 0 AND email IN ('yudao@iocoder.cn', 'yuanma@iocoder.cn');
UPDATE system_dept SET email = '', updater = 'migration', update_time = now()
WHERE deleted = 0 AND email = 'ry@qq.com';
