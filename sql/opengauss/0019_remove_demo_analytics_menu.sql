-- 0019: remove the Vben playground demo analytics page (fake 用户量/访问量
-- numbers) from the product navigation. The workspace stays the dashboard home.
UPDATE system_menu
SET deleted = 1, updater = 'migration', update_time = now()
WHERE deleted = 0 AND id = 30202 AND path = '/analytics';
