-- The consolidated 0001 snapshot accidentally retained runtime/demo
-- notifications. Remove only the exact known baseline records so upgraded
-- databases keep all real notifications. The predicates make this safe to
-- run repeatedly and avoid matching later messages that reuse a template.

DELETE FROM system_notify_message
WHERE (id, template_code) IN (
    (2, 'test'),
    (3, 'test'),
    (4, 'register'),
    (5, 'test'),
    (6, 'test'),
    (7, 'test'),
    (8, 'register'),
    (9, 'brokerage_withdraw_audit_approve'),
    (10, 'brokerage_withdraw_audit_approve'),
    (12, 'codex_test_1784183375')
);

DELETE FROM system_notify_template
WHERE id = 2 AND code = 'codex_test_1784183375';

UPDATE system_users
SET login_ip = '', login_date = NULL
WHERE id = 1
  AND username = 'admin'
  AND login_ip = ''
  AND login_date = TIMESTAMP '2026-07-21 05:55:59.786431';
