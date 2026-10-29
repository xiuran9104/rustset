-- Ticket expiry lifecycle. Reminders anchor on the ticket because the
-- approval is the source of the lifecycle permission; resources carry a
-- synced expire_at for display and auxiliary validation only — never a
-- second independent reminder.
ALTER TABLE public.infra_resource_ticket
    ADD COLUMN IF NOT EXISTS expiry_remind_stage smallint DEFAULT 0 NOT NULL;

ALTER TABLE public.infra_cloud_resource
    ADD COLUMN IF NOT EXISTS expire_at character varying(32);

ALTER TABLE public.infra_physical_resource
    ADD COLUMN IF NOT EXISTS expire_at character varying(32);

-- 0 = not reminded, 1 = T-3 sent, 2 = T-1 sent, 3 = expired and reclaimed.
COMMENT ON COLUMN public.infra_resource_ticket.expiry_remind_stage IS 'reminder progress: 0 none, 1 three-day, 2 one-day, 3 expired';

INSERT INTO public.system_notify_template
    (id, name, code, nickname, content, type, params, status, remark, creator, updater)
SELECT nextval('public.system_notify_template_seq'), '工单到期提醒', 'ticket_expire_reminder', '工单提醒',
       '工单「{ticketName}」将于 {expireAt} 到期（剩余 {daysLeft} 天）。如需继续使用请在工单系统提交延期申请；到期未续期将自动进入回收流程。',
       2, '["ticketName","expireAt","daysLeft"]', 0, '工单到期生命周期提醒', 'system', 'system'
WHERE NOT EXISTS (
    SELECT 1 FROM public.system_notify_template
    WHERE code = 'ticket_expire_reminder' AND deleted = 0
);
