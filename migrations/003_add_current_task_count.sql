-- Add current_task_count column to workers table
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'workers' AND column_name = 'current_task_count') THEN
        ALTER TABLE workers ADD COLUMN current_task_count INTEGER NOT NULL DEFAULT 0;
        
        -- Add check constraint to ensure non-negative value
        ALTER TABLE workers ADD CONSTRAINT workers_current_task_count_non_negative 
        CHECK (current_task_count >= 0);
    END IF;
END $$;