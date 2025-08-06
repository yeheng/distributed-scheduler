#!/bin/bash

# 分布式任务调度系统 - 认证示例脚本

BASE_URL="http://localhost:8080"

echo "=== 分布式任务调度系统认证测试 ==="
echo

# 1. 健康检查（无需认证）
echo "1. 健康检查（无需认证）"
curl -s "$BASE_URL/health" | jq .
echo

# 2. 用户登录
echo "2. 管理员登录"
LOGIN_RESPONSE=$(curl -s -X POST "$BASE_URL/api/auth/login" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "password": "admin123"
  }')

echo $LOGIN_RESPONSE | jq .
TOKEN=$(echo $LOGIN_RESPONSE | jq -r '.data.access_token')
echo "获取到的令牌: ${TOKEN:0:50}..."
echo

# 3. 验证令牌
echo "3. 验证JWT令牌"
curl -s -X GET "$BASE_URL/api/auth/validate" \
  -H "Authorization: Bearer $TOKEN" | jq .
echo

# 4. 使用JWT令牌访问受保护的端点
echo "4. 使用JWT令牌访问任务列表"
curl -s -X GET "$BASE_URL/api/tasks" \
  -H "Authorization: Bearer $TOKEN" | jq .
echo

# 5. 使用API Key访问
echo "5. 使用API密钥访问任务列表"
curl -s -X GET "$BASE_URL/api/tasks" \
  -H "X-API-Key: admin-api-key-example" | jq .
echo

# 6. 创建新的API密钥（需要Admin权限）
echo "6. 创建新的API密钥"
curl -s -X POST "$BASE_URL/api/auth/api-keys" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "new-operator-key",
    "permissions": ["TaskRead", "TaskWrite", "SystemRead"],
    "expires_in_days": 30
  }' | jq .
echo

# 7. 刷新令牌
echo "7. 刷新JWT令牌"
REFRESH_RESPONSE=$(curl -s -X POST "$BASE_URL/api/auth/refresh" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{}')

echo $REFRESH_RESPONSE | jq .
NEW_TOKEN=$(echo $REFRESH_RESPONSE | jq -r '.data.access_token')
echo

# 8. 尝试无权限操作（使用受限的操作员凭据）
echo "8. 操作员登录测试"
OPERATOR_RESPONSE=$(curl -s -X POST "$BASE_URL/api/auth/login" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "operator",
    "password": "op123"
  }')

OPERATOR_TOKEN=$(echo $OPERATOR_RESPONSE | jq -r '.data.access_token')

echo "尝试使用操作员权限创建API密钥（应该失败）"
curl -s -X POST "$BASE_URL/api/auth/api-keys" \
  -H "Authorization: Bearer $OPERATOR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "unauthorized-key",
    "permissions": ["Admin"]
  }' | jq .
echo

# 9. 登出
echo "9. 用户登出"
curl -s -X POST "$BASE_URL/api/auth/logout" \
  -H "Authorization: Bearer $TOKEN" | jq .
echo

# 10. 尝试使用已登出的令牌访问（应该失败）
echo "10. 尝试使用已登出的令牌访问（可能仍然有效，取决于实现）"
curl -s -X GET "$BASE_URL/api/tasks" \
  -H "Authorization: Bearer $TOKEN" | jq .
echo

echo "=== 认证测试完成 ==="