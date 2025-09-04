-- JWT Token Blacklist Table
-- 用于存储被撤销的JWT令牌，支持令牌黑名单和TTL自动清理

CREATE TABLE jwt_token_blacklist (
    id BIGSERIAL PRIMARY KEY,
    jti VARCHAR(255) NOT NULL UNIQUE, -- JWT令牌的唯一标识符
    user_id VARCHAR(255) NOT NULL,     -- 用户ID
    token_type VARCHAR(20) NOT NULL,   -- 令牌类型: 'access' 或 'refresh'
    revoked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), -- 撤销时间
    expires_at TIMESTAMPTZ NOT NULL,   -- 令牌过期时间
    reason TEXT,                        -- 撤销原因
    created_by VARCHAR(255),           -- 撤销操作者
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT jwt_token_blacklist_token_type_check CHECK (token_type IN ('access', 'refresh')),
    CONSTRAINT jwt_token_blacklist_expires_after_revoked CHECK (expires_at > revoked_at)
);

-- 创建索引以提高查询性能
CREATE INDEX idx_jwt_blacklist_jti ON jwt_token_blacklist(jti);
CREATE INDEX idx_jwt_blacklist_user_id ON jwt_token_blacklist(user_id);
CREATE INDEX idx_jwt_blacklist_expires_at ON jwt_token_blacklist(expires_at);
CREATE INDEX idx_jwt_blacklist_token_type ON jwt_token_blacklist(token_type);

-- 创建复合索引用于常见的查询模式
CREATE INDEX idx_jwt_blacklist_user_type ON jwt_token_blacklist(user_id, token_type);

-- 添加注释
COMMENT ON TABLE jwt_token_blacklist IS 'JWT令牌黑名单表，用于存储被撤销的JWT访问令牌和刷新令牌';
COMMENT ON COLUMN jwt_token_blacklist.jti IS 'JWT令牌的唯一标识符(jti claim)';
COMMENT ON COLUMN jwt_token_blacklist.user_id IS '关联的用户ID';
COMMENT ON COLUMN jwt_token_blacklist.token_type IS '令牌类型: access(访问令牌) 或 refresh(刷新令牌)';
COMMENT ON COLUMN jwt_token_blacklist.revoked_at IS '令牌被撤销的时间';
COMMENT ON COLUMN jwt_token_blacklist.expires_at IS '令牌的原始过期时间';
COMMENT ON COLUMN jwt_token_blacklist.reason IS '撤销原因，如: 手动撤销、密码修改、安全风险等';