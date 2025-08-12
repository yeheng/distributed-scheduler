use validator::ValidationError;

/// 验证worker ID格式
pub fn validate_worker_id(worker_id: &str) -> Result<(), ValidationError> {
    if worker_id.trim().is_empty() {
        return Err(ValidationError::new("Worker ID不能为空"));
    }
    
    if worker_id.len() > 255 {
        return Err(ValidationError::new("Worker ID长度不能超过255个字符"));
    }
    
    // Worker ID应该只包含字母、数字、下划线、中划线
    if !worker_id.chars().all(|c| {
        c.is_alphanumeric() || c == '_' || c == '-'
    }) {
        return Err(ValidationError::new("Worker ID只能包含字母、数字、下划线和连字符"));
    }
    
    Ok(())
}

/// 验证worker状态
pub fn validate_worker_status(status: &str) -> Result<(), ValidationError> {
    if status.trim().is_empty() {
        return Ok(()); // 空值是允许的，表示不过滤
    }
    
    match status.to_uppercase().as_str() {
        "ALIVE" | "DOWN" | "BUSY" | "IDLE" => Ok(()),
        _ => Err(ValidationError::new("无效的Worker状态，只能是 ALIVE, DOWN, BUSY 或 IDLE")),
    }
}

/// 验证分页参数
pub fn validate_pagination(page: Option<i64>, page_size: Option<i64>) -> Result<(i64, i64), ValidationError> {
    let validated_page = page.unwrap_or(1).max(1);
    let validated_page_size = page_size.unwrap_or(20);
    
    if validated_page_size > 100 {
        return Err(ValidationError::new("页面大小不能超过100"));
    }
    
    if validated_page_size < 1 {
        return Err(ValidationError::new("页面大小不能小于1"));
    }
    
    Ok((validated_page, validated_page_size))
}

/// 检查是否包含XSS风险
pub fn contains_xss_risk(input: &str) -> bool {
    let dangerous_patterns = [
        "<script", "</script>", "javascript:", "vbscript:", "onload=", "onerror=", 
        "onclick=", "onmouseover=", "onfocus=", "onblur=", "eval(", "document.cookie"
    ];
    
    let input_lower = input.to_lowercase();
    dangerous_patterns.iter().any(|&pattern| input_lower.contains(pattern))
}

/// 清理和规范化输入
pub fn sanitize_input(input: &str) -> String {
    // 移除首尾空白，并将多个连续空格替换为单个空格
    let cleaned = input.trim().split_whitespace().collect::<Vec<_>>().join(" ");
    
    // 移除或转义潜在危险字符
    cleaned
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('&', "&amp;")
}