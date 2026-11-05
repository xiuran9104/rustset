-- Durable scan queue. Workers claim rows with SKIP LOCKED and renew a lease;
-- expired work is retryable after a process crash or deployment restart.
DO $$ BEGIN ALTER TABLE public.infra_task ADD COLUMN payload jsonb NOT NULL DEFAULT '{}'::jsonb; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE public.infra_task ADD COLUMN idempotency_key character varying(128); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE public.infra_task ADD COLUMN attempt_count integer NOT NULL DEFAULT 0; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE public.infra_task ADD COLUMN max_attempts integer NOT NULL DEFAULT 3; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE public.infra_task ADD COLUMN timeout_seconds integer NOT NULL DEFAULT 300; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE public.infra_task ADD COLUMN next_attempt_at timestamp with time zone NOT NULL DEFAULT now(); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE public.infra_task ADD COLUMN lease_owner character varying(128); EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE public.infra_task ADD COLUMN lease_expires_at timestamp with time zone; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE public.infra_task ADD COLUMN heartbeat_at timestamp with time zone; EXCEPTION WHEN duplicate_column THEN NULL; END $$;
DO $$ BEGIN ALTER TABLE public.infra_task ADD COLUMN cancel_requested boolean NOT NULL DEFAULT false; EXCEPTION WHEN duplicate_column THEN NULL; END $$;

UPDATE public.infra_task
SET status = 'queued', next_attempt_at = now()
WHERE status = 'pending' AND deleted = 0;

CREATE UNIQUE INDEX IF NOT EXISTS idx_infra_task_tenant_idempotency
    ON public.infra_task(tenant_id, idempotency_key)
    WHERE deleted = 0 AND idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_infra_task_worker_queue
    ON public.infra_task(next_attempt_at, create_time)
    WHERE deleted = 0 AND status IN ('queued', 'retrying');

CREATE INDEX IF NOT EXISTS idx_infra_task_expired_lease
    ON public.infra_task(lease_expires_at)
    WHERE deleted = 0 AND status = 'running';

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.infra_task'::regclass
          AND conname = 'infra_task_retry_bounds_check'
    ) THEN
        ALTER TABLE public.infra_task ADD CONSTRAINT infra_task_retry_bounds_check
            CHECK (attempt_count >= 0 AND max_attempts BETWEEN 1 AND 20);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'public.infra_task'::regclass
          AND conname = 'infra_task_timeout_bounds_check'
    ) THEN
        ALTER TABLE public.infra_task ADD CONSTRAINT infra_task_timeout_bounds_check
            CHECK (timeout_seconds BETWEEN 1 AND 86400);
    END IF;
END $$;
