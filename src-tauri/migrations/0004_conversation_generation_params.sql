-- Fase 5.4: per-conversation generation parameters.
-- NULL = inherit SamplingParams::default() / DEFAULT_COMPLETION_BUDGET.
ALTER TABLE conversations ADD COLUMN temperature REAL;
ALTER TABLE conversations ADD COLUMN top_p REAL;
ALTER TABLE conversations ADD COLUMN top_k INTEGER;
ALTER TABLE conversations ADD COLUMN repeat_penalty REAL;
ALTER TABLE conversations ADD COLUMN repeat_last_n INTEGER;
ALTER TABLE conversations ADD COLUMN seed INTEGER;
ALTER TABLE conversations ADD COLUMN max_completion_tokens INTEGER;
