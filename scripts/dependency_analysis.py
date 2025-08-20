#!/usr/bin/env python3
"""
依赖关系分析脚本 - 生成依赖关系图并验证架构合规性
"""

import os
import re
import sys
import json
import subprocess
from pathlib import Path
from typing import Dict, List, Set, Tuple
import argparse

class DependencyAnalyzer:
    def __init__(self, project_root: str):
        self.project_root = Path(project_root)
        self.crates_dir = self.project_root / "crates"
        self.dependencies: Dict[str, List[str]] = {}
        self.reverse_dependencies: Dict[str, List[str]] = {}
        
        # 定义架构层级（从低到高）
        self.architecture_layers = {
            "errors": 0,
            "config": 0,
            "domain": 1,
            "core": 2,
            "application": 3,
            "infrastructure": 3,
            "observability": 3,
            "testing-utils": 3,
            "dispatcher": 4,
            "worker": 4,
            "api": 4
        }
    
    def parse_cargo_toml(self, crate_path: Path) -> Dict[str, List[str]]:
        """解析Cargo.toml文件，提取内部依赖"""
        cargo_toml = crate_path / "Cargo.toml"
        if not cargo_toml.exists():
            return {}
        
        with open(cargo_toml, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # 提取scheduler内部依赖
        dependencies = []
        in_dependencies = False
        
        for line in content.split('\n'):
            line = line.strip()
            
            if line == '[dependencies]':
                in_dependencies = True
                continue
            elif line.startswith('[') and line != '[dependencies]':
                in_dependencies = False
                continue
            
            if in_dependencies and line:
                # 匹配形如 scheduler-xxx = { path = "../xxx" } 的依赖
                match = re.match(r'(scheduler-\w+)\s*=.*path\s*=\s*"\.\./([\w-]+)"', line)
                if match:
                    dep_name = match.group(1)
                    dep_crate = match.group(2)
                    dependencies.append(dep_crate)
        
        return dependencies
    
    def analyze_dependencies(self):
        """分析所有crate的依赖关系"""
        for crate_dir in self.crates_dir.iterdir():
            if not crate_dir.is_dir():
                continue
                
            crate_name = crate_dir.name
            deps = self.parse_cargo_toml(crate_dir)
            self.dependencies[crate_name] = deps
            
            # 构建反向依赖关系
            for dep in deps:
                if dep not in self.reverse_dependencies:
                    self.reverse_dependencies[dep] = []
                self.reverse_dependencies[dep].append(crate_name)
    
    def find_cycles(self) -> List[List[str]]:
        """检测循环依赖"""
        cycles = []
        visited = set()
        rec_stack = set()
        path = []
        
        def dfs(crate: str):
            if crate in rec_stack:
                # 找到循环，构建循环路径
                cycle_start = path.index(crate)
                cycle = path[cycle_start:] + [crate]
                cycles.append(cycle)
                return
            
            if crate in visited:
                return
            
            visited.add(crate)
            rec_stack.add(crate)
            path.append(crate)
            
            for dep in self.dependencies.get(crate, []):
                dfs(dep)
            
            path.pop()
            rec_stack.remove(crate)
        
        for crate in self.dependencies:
            if crate not in visited:
                dfs(crate)
        
        return cycles
    
    def validate_layer_dependencies(self) -> List[str]:
        """验证层级依赖关系是否符合架构要求"""
        violations = []
        
        for crate, deps in self.dependencies.items():
            if crate not in self.architecture_layers:
                violations.append(f"未知crate: {crate}")
                continue
                
            crate_layer = self.architecture_layers[crate]
            
            for dep in deps:
                if dep not in self.architecture_layers:
                    violations.append(f"未知依赖: {crate} -> {dep}")
                    continue
                    
                dep_layer = self.architecture_layers[dep]
                
                # 只能依赖同级或更低级别的层
                if dep_layer > crate_layer:
                    violations.append(
                        f"架构违规: {crate}(Layer {crate_layer}) 不能依赖 "
                        f"{dep}(Layer {dep_layer})"
                    )
        
        return violations
    
    def generate_graphviz(self) -> str:
        """生成Graphviz DOT格式的依赖图"""
        dot_content = ["digraph dependencies {"]
        dot_content.append("  rankdir=TB;")
        dot_content.append("  node [shape=box, style=rounded];")
        
        # 按层级分组
        layer_groups = {}
        for crate, layer in self.architecture_layers.items():
            if layer not in layer_groups:
                layer_groups[layer] = []
            layer_groups[layer].append(crate)
        
        # 为不同层级设置不同颜色
        layer_colors = ["lightblue", "lightgreen", "lightyellow", "lightcoral", "lightpink"]
        
        for layer, crates in layer_groups.items():
            color = layer_colors[layer % len(layer_colors)]
            for crate in crates:
                if crate in self.dependencies:  # 只显示存在的crate
                    dot_content.append(f'  "{crate}" [fillcolor={color}, style=filled];')
        
        # 添加依赖关系
        for crate, deps in self.dependencies.items():
            for dep in deps:
                dot_content.append(f'  "{crate}" -> "{dep}";')
        
        dot_content.append("}")
        return "\n".join(dot_content)
    
    def generate_mermaid(self) -> str:
        """生成Mermaid格式的依赖图"""
        mermaid_content = ["graph TD"]
        
        # 添加依赖关系
        for crate, deps in self.dependencies.items():
            for dep in deps:
                mermaid_content.append(f'  {crate} --> {dep}')
        
        # 添加样式
        mermaid_content.extend([
            "",
            "  classDef layer0 fill:#e1f5fe",
            "  classDef layer1 fill:#e8f5e8", 
            "  classDef layer2 fill:#fff3e0",
            "  classDef layer3 fill:#fce4ec",
            "  classDef layer4 fill:#f3e5f5"
        ])
        
        # 应用样式
        for crate, layer in self.architecture_layers.items():
            if crate in self.dependencies:
                mermaid_content.append(f'  class {crate} layer{layer}')
        
        return "\n".join(mermaid_content)
    
    def generate_report(self) -> Dict:
        """生成完整的依赖分析报告"""
        cycles = self.find_cycles()
        violations = self.validate_layer_dependencies()
        
        # 计算一些统计信息
        total_crates = len(self.dependencies)
        total_dependencies = sum(len(deps) for deps in self.dependencies.values())
        avg_dependencies = total_dependencies / total_crates if total_crates > 0 else 0
        
        return {
            "summary": {
                "total_crates": total_crates,
                "total_dependencies": total_dependencies,
                "average_dependencies_per_crate": round(avg_dependencies, 2),
                "cycles_found": len(cycles),
                "architecture_violations": len(violations)
            },
            "dependencies": self.dependencies,
            "reverse_dependencies": self.reverse_dependencies,
            "cycles": cycles,
            "architecture_violations": violations,
            "layer_mapping": self.architecture_layers
        }
    
    def save_dependency_graph(self, output_dir: Path):
        """保存依赖关系图到文件"""
        output_dir.mkdir(exist_ok=True)
        
        # 保存Graphviz格式
        with open(output_dir / "dependencies.dot", 'w') as f:
            f.write(self.generate_graphviz())
        
        # 保存Mermaid格式
        with open(output_dir / "dependencies.mmd", 'w') as f:
            f.write(self.generate_mermaid())
        
        # 尝试生成PNG图片（如果系统有dot命令）
        try:
            subprocess.run([
                "dot", "-Tpng", 
                str(output_dir / "dependencies.dot"), 
                "-o", str(output_dir / "dependencies.png")
            ], check=True, capture_output=True)
            print(f"✅ 依赖关系图已生成: {output_dir / 'dependencies.png'}")
        except (subprocess.CalledProcessError, FileNotFoundError):
            print("ℹ️  无法生成PNG图片，请安装Graphviz工具")


def main():
    parser = argparse.ArgumentParser(description="分析Rust项目的依赖关系")
    parser.add_argument("--project-root", "-p", 
                       default=".", 
                       help="项目根目录路径")
    parser.add_argument("--output-dir", "-o", 
                       default="./docs/architecture", 
                       help="输出目录")
    parser.add_argument("--format", "-f", 
                       choices=["json", "text", "both"], 
                       default="both",
                       help="输出格式")
    
    args = parser.parse_args()
    
    analyzer = DependencyAnalyzer(args.project_root)
    analyzer.analyze_dependencies()
    
    report = analyzer.generate_report()
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # 打印简要报告
    print("🔍 依赖关系分析结果:")
    print(f"📦 总计crate数量: {report['summary']['total_crates']}")
    print(f"🔗 总计依赖关系: {report['summary']['total_dependencies']}")
    print(f"📊 平均每个crate的依赖数: {report['summary']['average_dependencies_per_crate']}")
    
    if report['summary']['cycles_found'] > 0:
        print(f"🚨 发现循环依赖: {report['summary']['cycles_found']} 个")
        for i, cycle in enumerate(report['cycles']):
            print(f"  循环 {i+1}: {' -> '.join(cycle)}")
    else:
        print("✅ 未发现循环依赖")
    
    if report['summary']['architecture_violations'] > 0:
        print(f"🚨 架构违规: {report['summary']['architecture_violations']} 个")
        for violation in report['architecture_violations']:
            print(f"  {violation}")
    else:
        print("✅ 架构依赖关系符合要求")
    
    # 保存报告
    if args.format in ["json", "both"]:
        with open(output_dir / "dependency_report.json", 'w', encoding='utf-8') as f:
            json.dump(report, f, indent=2, ensure_ascii=False)
        print(f"📄 JSON报告已保存: {output_dir / 'dependency_report.json'}")
    
    if args.format in ["text", "both"]:
        with open(output_dir / "dependency_report.txt", 'w', encoding='utf-8') as f:
            f.write("依赖关系分析报告\n")
            f.write("=" * 50 + "\n\n")
            
            f.write(f"总计crate数量: {report['summary']['total_crates']}\n")
            f.write(f"总计依赖关系: {report['summary']['total_dependencies']}\n")
            f.write(f"平均每个crate的依赖数: {report['summary']['average_dependencies_per_crate']}\n\n")
            
            f.write("各crate依赖关系:\n")
            f.write("-" * 30 + "\n")
            for crate, deps in report['dependencies'].items():
                f.write(f"{crate}: {', '.join(deps) if deps else '无依赖'}\n")
            
            if report['cycles']:
                f.write("\n循环依赖:\n")
                f.write("-" * 30 + "\n")
                for i, cycle in enumerate(report['cycles']):
                    f.write(f"循环 {i+1}: {' -> '.join(cycle)}\n")
            
            if report['architecture_violations']:
                f.write("\n架构违规:\n")
                f.write("-" * 30 + "\n")
                for violation in report['architecture_violations']:
                    f.write(f"{violation}\n")
        
        print(f"📄 文本报告已保存: {output_dir / 'dependency_report.txt'}")
    
    # 保存依赖图
    analyzer.save_dependency_graph(output_dir)
    
    # 返回退出码
    if report['summary']['cycles_found'] > 0 or report['summary']['architecture_violations'] > 0:
        print("\n❌ 发现架构问题，请修复后重试")
        sys.exit(1)
    else:
        print("\n✅ 架构依赖关系检查通过")
        sys.exit(0)


if __name__ == "__main__":
    main()