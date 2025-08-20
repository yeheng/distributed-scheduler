#!/bin/bash
# 架构合规性检查脚本 - CI/CD集成
# 用于验证项目架构依赖关系是否符合设计要求

set -euo pipefail

# 脚本配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
ANALYSIS_SCRIPT="${SCRIPT_DIR}/dependency_analysis.py"
OUTPUT_DIR="${PROJECT_ROOT}/docs/architecture"
TEMP_DIR="/tmp/architecture-check-$$"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查依赖工具
check_dependencies() {
    log_info "检查依赖工具..."
    
    if ! command -v python3 &> /dev/null; then
        log_error "Python3 未安装或不在PATH中"
        exit 1
    fi
    
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo 未安装或不在PATH中"
        exit 1
    fi
    
    log_success "依赖工具检查通过"
}

# 验证项目结构
validate_project_structure() {
    log_info "验证项目结构..."
    
    if [[ ! -f "${PROJECT_ROOT}/Cargo.toml" ]]; then
        log_error "未找到项目根目录的Cargo.toml文件"
        exit 1
    fi
    
    if [[ ! -d "${PROJECT_ROOT}/crates" ]]; then
        log_error "未找到crates目录"
        exit 1
    fi
    
    log_success "项目结构验证通过"
}

# 编译检查
compile_check() {
    log_info "编译检查..."
    
    cd "${PROJECT_ROOT}"
    
    if ! cargo check --all-targets --quiet 2>/dev/null; then
        log_error "编译检查失败"
        log_info "运行详细编译检查..."
        cargo check --all-targets
        exit 1
    fi
    
    log_success "编译检查通过"
}

# 运行依赖分析
run_dependency_analysis() {
    log_info "运行依赖关系分析..."
    
    # 创建临时目录
    mkdir -p "${TEMP_DIR}"
    
    # 运行分析脚本
    if python3 "${ANALYSIS_SCRIPT}" \
        --project-root "${PROJECT_ROOT}" \
        --output-dir "${TEMP_DIR}" \
        --format json; then
        log_success "依赖关系分析通过"
        
        # 复制结果到输出目录
        mkdir -p "${OUTPUT_DIR}"
        cp "${TEMP_DIR}"/dependency_report.* "${OUTPUT_DIR}/" 2>/dev/null || true
        cp "${TEMP_DIR}"/dependencies.* "${OUTPUT_DIR}/" 2>/dev/null || true
        
        return 0
    else
        log_error "依赖关系分析失败"
        
        # 显示分析结果
        if [[ -f "${TEMP_DIR}/dependency_report.json" ]]; then
            log_info "分析报告详情:"
            python3 -c "
import json
with open('${TEMP_DIR}/dependency_report.json') as f:
    data = json.load(f)
    
if data['summary']['cycles_found'] > 0:
    print('🚨 循环依赖:')
    for i, cycle in enumerate(data['cycles']):
        print(f'  循环 {i+1}: {\" -> \".join(cycle)}')

if data['summary']['architecture_violations'] > 0:
    print('🚨 架构违规:')
    for violation in data['architecture_violations']:
        print(f'  {violation}')
"
        fi
        
        return 1
    fi
}

# 生成架构图
generate_architecture_diagrams() {
    log_info "生成架构图..."
    
    # 检查是否有Graphviz
    if command -v dot &> /dev/null; then
        if [[ -f "${OUTPUT_DIR}/dependencies.dot" ]]; then
            dot -Tpng "${OUTPUT_DIR}/dependencies.dot" -o "${OUTPUT_DIR}/dependencies.png"
            dot -Tsvg "${OUTPUT_DIR}/dependencies.dot" -o "${OUTPUT_DIR}/dependencies.svg"
            log_success "架构图生成完成: dependencies.png, dependencies.svg"
        fi
    else
        log_warning "Graphviz未安装，跳过PNG/SVG图片生成"
    fi
    
    # Mermaid图总是可以生成的
    if [[ -f "${OUTPUT_DIR}/dependencies.mmd" ]]; then
        log_success "Mermaid图已生成: dependencies.mmd"
    fi
}

# 验证架构规则
validate_architecture_rules() {
    log_info "验证架构规则..."
    
    local rules_passed=true
    
    # 规则1: 检查是否还有foundation残留
    if find "${PROJECT_ROOT}/crates" -name "*.toml" -exec grep -l "scheduler-foundation" {} \; 2>/dev/null | head -1 | grep -q .; then
        log_error "规则违反: 发现foundation crate残留引用"
        rules_passed=false
    fi
    
    # 规则2: 检查domain层是否只依赖errors
    domain_deps=$(grep -A20 '^\[dependencies\]' "${PROJECT_ROOT}/crates/domain/Cargo.toml" | \
                  grep -E 'scheduler-\w+' | grep -v 'scheduler-errors' | wc -l)
    if [[ $domain_deps -gt 0 ]]; then
        log_error "规则违反: domain层不应依赖除errors外的其他scheduler crates"
        rules_passed=false
    fi
    
    # 规则3: 检查循环依赖
    if [[ -f "${OUTPUT_DIR}/dependency_report.json" ]]; then
        cycles=$(python3 -c "
import json
with open('${OUTPUT_DIR}/dependency_report.json') as f:
    print(json.load(f)['summary']['cycles_found'])
")
        if [[ $cycles -gt 0 ]]; then
            log_error "规则违反: 发现 ${cycles} 个循环依赖"
            rules_passed=false
        fi
    fi
    
    if $rules_passed; then
        log_success "架构规则验证通过"
        return 0
    else
        log_error "架构规则验证失败"
        return 1
    fi
}

# 生成合规性报告
generate_compliance_report() {
    local status=$1
    local report_file="${OUTPUT_DIR}/compliance_report.json"
    
    log_info "生成合规性报告..."
    
    mkdir -p "${OUTPUT_DIR}"
    
    cat > "${report_file}" << EOF
{
  "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "status": "${status}",
  "checks": {
    "project_structure": "passed",
    "compilation": "passed",
    "dependency_analysis": "${status}",
    "architecture_rules": "${status}"
  },
  "artifacts": {
    "dependency_graph": "dependencies.mmd",
    "analysis_report": "dependency_report.json",
    "compliance_report": "compliance_report.json"
  }
}
EOF
    
    log_success "合规性报告已生成: ${report_file}"
}

# 清理临时文件
cleanup() {
    if [[ -d "${TEMP_DIR}" ]]; then
        rm -rf "${TEMP_DIR}"
    fi
}

# 主函数
main() {
    log_info "开始架构合规性检查..."
    log_info "项目根目录: ${PROJECT_ROOT}"
    
    # 设置清理钩子
    trap cleanup EXIT
    
    # 执行检查步骤
    check_dependencies
    validate_project_structure
    compile_check
    
    local overall_status="passed"
    
    if ! run_dependency_analysis; then
        overall_status="failed"
    fi
    
    if ! validate_architecture_rules; then
        overall_status="failed"
    fi
    
    generate_architecture_diagrams
    generate_compliance_report "${overall_status}"
    
    if [[ "${overall_status}" == "passed" ]]; then
        log_success "架构合规性检查通过 ✅"
        exit 0
    else
        log_error "架构合规性检查失败 ❌"
        exit 1
    fi
}

# 帮助信息
show_help() {
    cat << EOF
架构合规性检查脚本

用法: $0 [选项]

选项:
  -h, --help     显示此帮助信息
  -v, --verbose  启用详细输出

描述:
  此脚本验证Rust项目的架构依赖关系是否符合设计要求，
  包括循环依赖检测、层级依赖验证和架构规则检查。

输出:
  - docs/architecture/dependency_report.json - 详细分析报告
  - docs/architecture/dependencies.mmd - Mermaid依赖图
  - docs/architecture/compliance_report.json - 合规性报告
  
退出码:
  0 - 所有检查通过
  1 - 发现架构问题

示例:
  $0                    # 运行完整检查
  $0 --verbose         # 详细输出模式
EOF
}

# 解析命令行参数
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        -v|--verbose)
            set -x
            shift
            ;;
        *)
            log_error "未知选项: $1"
            show_help
            exit 1
            ;;
    esac
done

# 运行主函数
main "$@"