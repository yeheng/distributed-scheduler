//! # Workflow Generation Engine
//! 
//! Implementation workflow generator for analyzing PRDs and feature specifications
//! to generate comprehensive, step-by-step implementation workflows with expert guidance.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::{Result, Context};

/// Workflow generation strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowStrategy {
    /// Sequential phases with clear deliverables
    Systematic,
    /// Sprint-based iterative development
    Agile,
    /// Minimum viable product focus
    MVP,
}

/// Expert persona types for specialized workflow generation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExpertPersona {
    Architect,
    Frontend,
    Backend,
    Security,
    DevOps,
    QA,
    Performance,
    Analyst,
    Mentor,
    Scribe,
}

/// Output format options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OutputFormat {
    Roadmap,
    Tasks,
    Detailed,
}

/// Risk assessment level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Dependency relationship types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DependencyType {
    /// Internal code dependencies
    Internal,
    /// External service dependencies
    External,
    /// Framework/technology dependencies
    Technical,
    /// Cross-team coordination dependencies
    Team,
    /// Infrastructure requirements
    Infrastructure,
}

/// Configuration for workflow generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    pub strategy: WorkflowStrategy,
    pub persona: Option<ExpertPersona>,
    pub output_format: OutputFormat,
    pub include_estimates: bool,
    pub include_dependencies: bool,
    pub include_risks: bool,
    pub identify_parallel: bool,
    pub create_milestones: bool,
    pub enable_context7: bool,
    pub enable_sequential: bool,
    pub enable_magic: bool,
    pub enable_all_mcp: bool,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            strategy: WorkflowStrategy::Systematic,
            persona: None,
            output_format: OutputFormat::Roadmap,
            include_estimates: false,
            include_dependencies: false,
            include_risks: false,
            identify_parallel: false,
            create_milestones: false,
            enable_context7: false,
            enable_sequential: false,
            enable_magic: false,
            enable_all_mcp: false,
        }
    }
}

/// Task complexity estimation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityEstimate {
    pub level: String,
    pub time_estimate: String,
    pub confidence: f32,
    pub factors: Vec<String>,
}

/// Risk assessment for workflow items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub level: RiskLevel,
    pub description: String,
    pub probability: f32,
    pub impact: f32,
    pub mitigation_strategies: Vec<String>,
}

/// Dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub dependency_type: DependencyType,
    pub description: String,
    pub criticality: RiskLevel,
    pub estimated_effort: Option<String>,
}

/// Individual workflow task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTask {
    pub id: String,
    pub name: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub dependencies: Vec<String>,
    pub complexity: Option<ComplexityEstimate>,
    pub risks: Vec<RiskAssessment>,
    pub parallelizable: bool,
    pub estimated_hours: Option<u32>,
    pub persona: Option<ExpertPersona>,
    pub tools_required: Vec<String>,
    pub validation_steps: Vec<String>,
}

/// Workflow phase containing multiple tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPhase {
    pub name: String,
    pub description: String,
    pub duration_estimate: Option<String>,
    pub tasks: Vec<WorkflowTask>,
    pub deliverables: Vec<String>,
    pub success_criteria: Vec<String>,
    pub risks: Vec<RiskAssessment>,
}

/// Complete workflow specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSpecification {
    pub title: String,
    pub description: String,
    pub strategy: WorkflowStrategy,
    pub primary_persona: Option<ExpertPersona>,
    pub phases: Vec<WorkflowPhase>,
    pub overall_dependencies: Vec<Dependency>,
    pub parallel_work_streams: Vec<Vec<String>>, // Task IDs that can run in parallel
    pub milestones: Vec<Milestone>,
    pub success_metrics: Vec<String>,
    pub total_estimated_duration: Option<String>,
}

/// Project milestone
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub name: String,
    pub description: String,
    pub completion_criteria: Vec<String>,
    pub target_phase: String,
    pub dependencies: Vec<String>,
}

/// PRD parsing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRDAnalysis {
    pub title: String,
    pub objectives: Vec<String>,
    pub requirements: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub constraints: Vec<String>,
    pub stakeholders: Vec<String>,
    pub success_metrics: Vec<String>,
    pub domain_indicators: Vec<ExpertPersona>,
    pub complexity_indicators: Vec<String>,
    pub technology_stack: Vec<String>,
}

/// Main workflow generator engine
#[derive(Debug)]
pub struct WorkflowGenerator {
    config: WorkflowConfig,
    persona_templates: HashMap<ExpertPersona, PersonaTemplate>,
}

/// Persona-specific workflow template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaTemplate {
    pub focus_areas: Vec<String>,
    pub quality_standards: Vec<String>,
    pub preferred_tools: Vec<String>,
    pub risk_considerations: Vec<String>,
    pub validation_approaches: Vec<String>,
}

impl WorkflowGenerator {
    /// Create a new workflow generator with the given configuration
    pub fn new(config: WorkflowConfig) -> Self {
        let mut generator = Self {
            config,
            persona_templates: HashMap::new(),
        };
        generator.initialize_persona_templates();
        generator
    }

    /// Generate workflow from PRD file or feature description
    pub async fn generate_workflow(
        &self,
        input: &str,
    ) -> Result<WorkflowSpecification> {
        // Determine if input is a file path or direct description
        let prd_analysis = if PathBuf::from(input).exists() {
            self.parse_prd_file(input).await?
        } else {
            self.analyze_feature_description(input).await?
        };

        // Auto-detect persona if not specified
        let primary_persona = self.config.persona
            .or_else(|| self.detect_primary_persona(&prd_analysis));

        // Generate workflow based on strategy
        let workflow = match self.config.strategy {
            WorkflowStrategy::Systematic => self.generate_systematic_workflow(&prd_analysis, primary_persona).await?,
            WorkflowStrategy::Agile => self.generate_agile_workflow(&prd_analysis, primary_persona).await?,
            WorkflowStrategy::MVP => self.generate_mvp_workflow(&prd_analysis, primary_persona).await?,
        };

        Ok(workflow)
    }

    /// Parse PRD from markdown file
    async fn parse_prd_file(&self, file_path: &str) -> Result<PRDAnalysis> {
        let content = tokio::fs::read_to_string(file_path)
            .await
            .context("Failed to read PRD file")?;

        self.parse_prd_content(&content).await
    }

    /// Parse PRD content from string
    async fn parse_prd_content(&self, content: &str) -> Result<PRDAnalysis> {
        // Enhanced PRD parsing logic
        let mut analysis = PRDAnalysis {
            title: String::new(),
            objectives: Vec::new(),
            requirements: Vec::new(),
            acceptance_criteria: Vec::new(),
            constraints: Vec::new(),
            stakeholders: Vec::new(),
            success_metrics: Vec::new(),
            domain_indicators: Vec::new(),
            complexity_indicators: Vec::new(),
            technology_stack: Vec::new(),
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut current_section = "";

        for line in lines {
            let trimmed = line.trim();
            
            // Extract title
            if trimmed.starts_with("# ") {
                analysis.title = trimmed[2..].to_string();
                continue;
            }

            // Section headers
            if trimmed.starts_with("## ") {
                current_section = trimmed;
                continue;
            }

            // List items
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                let content = trimmed[2..].to_string();
                match current_section {
                    "## Objectives" | "## Goals" => analysis.objectives.push(content),
                    "## Requirements" | "## Functional Requirements" => analysis.requirements.push(content),
                    "## Acceptance Criteria" => analysis.acceptance_criteria.push(content),
                    "## Constraints" | "## Limitations" => analysis.constraints.push(content),
                    "## Stakeholders" => analysis.stakeholders.push(content),
                    "## Success Metrics" | "## KPIs" => analysis.success_metrics.push(content),
                    _ => {}
                }
            }
        }

        // Analyze domain indicators
        analysis.domain_indicators = self.detect_domain_indicators(&content);
        analysis.complexity_indicators = self.detect_complexity_indicators(&content);
        analysis.technology_stack = self.detect_technology_stack(&content);

        Ok(analysis)
    }

    /// Analyze feature description for workflow generation
    async fn analyze_feature_description(&self, description: &str) -> Result<PRDAnalysis> {
        let mut analysis = PRDAnalysis {
            title: description.lines().next().unwrap_or("Feature Implementation").to_string(),
            objectives: vec![description.to_string()],
            requirements: self.extract_implied_requirements(description),
            acceptance_criteria: Vec::new(),
            constraints: Vec::new(),
            stakeholders: vec!["Development Team".to_string()],
            success_metrics: vec!["Feature implemented and tested".to_string()],
            domain_indicators: self.detect_domain_indicators(description),
            complexity_indicators: self.detect_complexity_indicators(description),
            technology_stack: self.detect_technology_stack(description),
        };

        // Generate basic acceptance criteria from description
        analysis.acceptance_criteria = self.generate_acceptance_criteria(description);

        Ok(analysis)
    }

    /// Detect primary persona based on PRD analysis
    fn detect_primary_persona(&self, analysis: &PRDAnalysis) -> Option<ExpertPersona> {
        let mut scores = HashMap::new();
        
        // Initialize scores
        for persona in [
            ExpertPersona::Architect,
            ExpertPersona::Frontend,
            ExpertPersona::Backend,
            ExpertPersona::Security,
            ExpertPersona::DevOps,
            ExpertPersona::QA,
            ExpertPersona::Performance,
            ExpertPersona::Analyst,
        ] {
            scores.insert(persona, 0i32);
        }

        // Score based on domain indicators
        for persona in &analysis.domain_indicators {
            *scores.entry(persona.clone()).or_insert(0) += 10;
        }

        // Analyze content for persona keywords
        let content = format!("{} {} {}", 
            analysis.title,
            analysis.objectives.join(" "),
            analysis.requirements.join(" ")
        ).to_lowercase();

        // Frontend indicators
        if content.contains("ui") || content.contains("component") || content.contains("frontend") ||
           content.contains("react") || content.contains("vue") || content.contains("angular") ||
           content.contains("responsive") || content.contains("accessibility") {
            *scores.entry(ExpertPersona::Frontend).or_insert(0) += 5;
        }

        // Backend indicators
        if content.contains("api") || content.contains("backend") || content.contains("database") ||
           content.contains("service") || content.contains("server") || content.contains("endpoint") {
            *scores.entry(ExpertPersona::Backend).or_insert(0) += 5;
        }

        // Security indicators
        if content.contains("security") || content.contains("auth") || content.contains("encrypt") ||
           content.contains("vulnerability") || content.contains("compliance") {
            *scores.entry(ExpertPersona::Security).or_insert(0) += 5;
        }

        // Architecture indicators
        if content.contains("architecture") || content.contains("design") || content.contains("scalability") ||
           content.contains("system") || content.contains("microservice") {
            *scores.entry(ExpertPersona::Architect).or_insert(0) += 5;
        }

        // DevOps indicators
        if content.contains("deploy") || content.contains("infrastructure") || content.contains("ci/cd") ||
           content.contains("docker") || content.contains("kubernetes") {
            *scores.entry(ExpertPersona::DevOps).or_insert(0) += 5;
        }

        // Performance indicators
        if content.contains("performance") || content.contains("optimize") || content.contains("scaling") ||
           content.contains("bottleneck") || content.contains("latency") {
            *scores.entry(ExpertPersona::Performance).or_insert(0) += 5;
        }

        // Return persona with highest score
        scores.into_iter()
            .max_by_key(|(_, score)| *score)
            .filter(|(_, score)| *score > 0)
            .map(|(persona, _)| persona)
    }

    /// Detect domain indicators in content
    fn detect_domain_indicators(&self, content: &str) -> Vec<ExpertPersona> {
        let mut indicators = Vec::new();
        let content_lower = content.to_lowercase();

        if content_lower.contains("ui") || content_lower.contains("frontend") || content_lower.contains("component") {
            indicators.push(ExpertPersona::Frontend);
        }
        if content_lower.contains("api") || content_lower.contains("backend") || content_lower.contains("database") {
            indicators.push(ExpertPersona::Backend);
        }
        if content_lower.contains("security") || content_lower.contains("auth") {
            indicators.push(ExpertPersona::Security);
        }
        if content_lower.contains("architecture") || content_lower.contains("system") {
            indicators.push(ExpertPersona::Architect);
        }
        if content_lower.contains("deploy") || content_lower.contains("infrastructure") {
            indicators.push(ExpertPersona::DevOps);
        }
        if content_lower.contains("performance") || content_lower.contains("optimize") {
            indicators.push(ExpertPersona::Performance);
        }
        if content_lower.contains("test") || content_lower.contains("quality") {
            indicators.push(ExpertPersona::QA);
        }

        indicators
    }

    /// Detect complexity indicators in content
    fn detect_complexity_indicators(&self, content: &str) -> Vec<String> {
        let mut indicators = Vec::new();
        let content_lower = content.to_lowercase();

        if content_lower.contains("distributed") || content_lower.contains("microservice") {
            indicators.push("Distributed Architecture".to_string());
        }
        if content_lower.contains("real-time") || content_lower.contains("streaming") {
            indicators.push("Real-time Processing".to_string());
        }
        if content_lower.contains("scale") || content_lower.contains("high-volume") {
            indicators.push("Scalability Requirements".to_string());
        }
        if content_lower.contains("integration") || content_lower.contains("third-party") {
            indicators.push("External Integration".to_string());
        }
        if content_lower.contains("migrate") || content_lower.contains("legacy") {
            indicators.push("Legacy System Integration".to_string());
        }

        indicators
    }

    /// Detect technology stack from content
    fn detect_technology_stack(&self, content: &str) -> Vec<String> {
        let mut stack = Vec::new();
        let content_lower = content.to_lowercase();

        // Frontend technologies
        if content_lower.contains("react") { stack.push("React".to_string()); }
        if content_lower.contains("vue") { stack.push("Vue.js".to_string()); }
        if content_lower.contains("angular") { stack.push("Angular".to_string()); }

        // Backend technologies
        if content_lower.contains("rust") { stack.push("Rust".to_string()); }
        if content_lower.contains("node") || content_lower.contains("express") { stack.push("Node.js".to_string()); }
        if content_lower.contains("python") || content_lower.contains("django") || content_lower.contains("fastapi") { 
            stack.push("Python".to_string()); 
        }

        // Databases
        if content_lower.contains("postgres") { stack.push("PostgreSQL".to_string()); }
        if content_lower.contains("mysql") { stack.push("MySQL".to_string()); }
        if content_lower.contains("mongodb") { stack.push("MongoDB".to_string()); }
        if content_lower.contains("redis") { stack.push("Redis".to_string()); }

        // Message queues
        if content_lower.contains("rabbitmq") { stack.push("RabbitMQ".to_string()); }
        if content_lower.contains("kafka") { stack.push("Apache Kafka".to_string()); }

        // Cloud/Infrastructure
        if content_lower.contains("docker") { stack.push("Docker".to_string()); }
        if content_lower.contains("kubernetes") { stack.push("Kubernetes".to_string()); }
        if content_lower.contains("aws") { stack.push("AWS".to_string()); }

        stack
    }

    /// Extract implied requirements from feature description
    fn extract_implied_requirements(&self, description: &str) -> Vec<String> {
        let mut requirements = Vec::new();
        
        // Based on the distributed task scheduler context
        if description.to_lowercase().contains("task") || description.to_lowercase().contains("schedule") {
            requirements.extend(vec![
                "Task definition and storage".to_string(),
                "Scheduling mechanism".to_string(),
                "Worker node management".to_string(),
                "Task execution monitoring".to_string(),
                "Error handling and retry logic".to_string(),
            ]);
        }

        if description.to_lowercase().contains("api") {
            requirements.extend(vec![
                "RESTful API endpoints".to_string(),
                "Input validation".to_string(),
                "Authentication and authorization".to_string(),
                "Error response handling".to_string(),
                "API documentation".to_string(),
            ]);
        }

        if description.to_lowercase().contains("database") {
            requirements.extend(vec![
                "Database schema design".to_string(),
                "Data migration scripts".to_string(),
                "Connection pooling".to_string(),
                "Transaction management".to_string(),
                "Data backup strategy".to_string(),
            ]);
        }

        if requirements.is_empty() {
            requirements.push("Core functionality implementation".to_string());
        }

        requirements
    }

    /// Generate acceptance criteria from description
    fn generate_acceptance_criteria(&self, description: &str) -> Vec<String> {
        let mut criteria = Vec::new();
        
        criteria.push("Feature implemented according to specifications".to_string());
        criteria.push("All tests pass successfully".to_string());
        criteria.push("Code follows project coding standards".to_string());
        criteria.push("Documentation is complete and accurate".to_string());
        criteria.push("Security considerations are addressed".to_string());
        criteria.push("Performance requirements are met".to_string());
        
        criteria
    }

    /// Generate systematic workflow strategy
    async fn generate_systematic_workflow(
        &self,
        prd_analysis: &PRDAnalysis,
        primary_persona: Option<ExpertPersona>,
    ) -> Result<WorkflowSpecification> {
        let mut phases = Vec::new();

        // Phase 1: Analysis and Planning
        phases.push(self.create_analysis_phase(prd_analysis)?);

        // Phase 2: Architecture and Design
        phases.push(self.create_architecture_phase(prd_analysis, primary_persona.as_ref())?);

        // Phase 3: Core Implementation
        phases.push(self.create_implementation_phase(prd_analysis, primary_persona.as_ref())?);

        // Phase 4: Integration and Testing
        phases.push(self.create_integration_phase(prd_analysis)?);

        // Phase 5: Deployment and Monitoring
        phases.push(self.create_deployment_phase(prd_analysis)?);

        let workflow = WorkflowSpecification {
            title: format!("Systematic Implementation: {}", prd_analysis.title),
            description: format!("Comprehensive systematic workflow for implementing {}", prd_analysis.title),
            strategy: WorkflowStrategy::Systematic,
            primary_persona,
            phases,
            overall_dependencies: self.identify_overall_dependencies(prd_analysis)?,
            parallel_work_streams: self.identify_parallel_streams(prd_analysis)?,
            milestones: self.create_systematic_milestones()?,
            success_metrics: prd_analysis.success_metrics.clone(),
            total_estimated_duration: Some("6-8 weeks".to_string()),
        };

        Ok(workflow)
    }

    /// Generate agile workflow strategy  
    async fn generate_agile_workflow(
        &self,
        prd_analysis: &PRDAnalysis,
        primary_persona: Option<ExpertPersona>,
    ) -> Result<WorkflowSpecification> {
        let mut phases = Vec::new();

        // Sprint 0: Setup and Planning
        phases.push(self.create_sprint_zero_phase(prd_analysis)?);

        // Sprint 1-3: Core Features
        phases.push(self.create_core_features_sprint(prd_analysis, primary_persona.as_ref())?);

        // Sprint 4-6: Enhancement and Polish
        phases.push(self.create_enhancement_sprint(prd_analysis, primary_persona.as_ref())?);

        let workflow = WorkflowSpecification {
            title: format!("Agile Implementation: {}", prd_analysis.title),
            description: format!("Sprint-based agile workflow for implementing {}", prd_analysis.title),
            strategy: WorkflowStrategy::Agile,
            primary_persona,
            phases,
            overall_dependencies: self.identify_overall_dependencies(prd_analysis)?,
            parallel_work_streams: self.identify_parallel_streams(prd_analysis)?,
            milestones: self.create_agile_milestones()?,
            success_metrics: prd_analysis.success_metrics.clone(),
            total_estimated_duration: Some("8-10 weeks".to_string()),
        };

        Ok(workflow)
    }

    /// Generate MVP workflow strategy
    async fn generate_mvp_workflow(
        &self,
        prd_analysis: &PRDAnalysis,
        primary_persona: Option<ExpertPersona>,
    ) -> Result<WorkflowSpecification> {
        let mut phases = Vec::new();

        // Phase 1: MVP Definition and Setup
        phases.push(self.create_mvp_definition_phase(prd_analysis)?);

        // Phase 2: Core MVP Implementation
        phases.push(self.create_mvp_implementation_phase(prd_analysis, primary_persona.as_ref())?);

        // Phase 3: MVP Validation and Launch
        phases.push(self.create_mvp_validation_phase(prd_analysis)?);

        let workflow = WorkflowSpecification {
            title: format!("MVP Implementation: {}", prd_analysis.title),
            description: format!("Minimum viable product workflow for {}", prd_analysis.title),
            strategy: WorkflowStrategy::MVP,
            primary_persona,
            phases,
            overall_dependencies: self.identify_mvp_dependencies(prd_analysis)?,
            parallel_work_streams: self.identify_parallel_streams(prd_analysis)?,
            milestones: self.create_mvp_milestones()?,
            success_metrics: vec![
                "MVP delivered on time".to_string(),
                "Core functionality validated".to_string(),
                "User feedback collected".to_string(),
            ],
            total_estimated_duration: Some("3-4 weeks".to_string()),
        };

        Ok(workflow)
    }

    /// Initialize persona-specific templates
    fn initialize_persona_templates(&mut self) {
        self.persona_templates.insert(
            ExpertPersona::Architect,
            PersonaTemplate {
                focus_areas: vec![
                    "System design and architecture".to_string(),
                    "Scalability and performance".to_string(),
                    "Technology stack selection".to_string(),
                    "Integration patterns".to_string(),
                ],
                quality_standards: vec![
                    "SOLID principles compliance".to_string(),
                    "Clean architecture patterns".to_string(),
                    "Comprehensive documentation".to_string(),
                    "Scalability considerations".to_string(),
                ],
                preferred_tools: vec![
                    "Architecture diagrams".to_string(),
                    "Design patterns".to_string(),
                    "Performance modeling".to_string(),
                ],
                risk_considerations: vec![
                    "Technical debt accumulation".to_string(),
                    "Scalability bottlenecks".to_string(),
                    "Integration complexity".to_string(),
                ],
                validation_approaches: vec![
                    "Architecture review".to_string(),
                    "Performance testing".to_string(),
                    "Scalability validation".to_string(),
                ],
            },
        );

        self.persona_templates.insert(
            ExpertPersona::Frontend,
            PersonaTemplate {
                focus_areas: vec![
                    "User interface design".to_string(),
                    "User experience optimization".to_string(),
                    "Accessibility compliance".to_string(),
                    "Performance optimization".to_string(),
                ],
                quality_standards: vec![
                    "WCAG 2.1 AA compliance".to_string(),
                    "Cross-browser compatibility".to_string(),
                    "Mobile responsiveness".to_string(),
                    "Performance budgets".to_string(),
                ],
                preferred_tools: vec![
                    "Component libraries".to_string(),
                    "Design systems".to_string(),
                    "Testing frameworks".to_string(),
                ],
                risk_considerations: vec![
                    "Browser compatibility issues".to_string(),
                    "Performance bottlenecks".to_string(),
                    "Accessibility violations".to_string(),
                ],
                validation_approaches: vec![
                    "Cross-browser testing".to_string(),
                    "Accessibility audits".to_string(),
                    "Performance monitoring".to_string(),
                ],
            },
        );

        self.persona_templates.insert(
            ExpertPersona::Backend,
            PersonaTemplate {
                focus_areas: vec![
                    "API design and implementation".to_string(),
                    "Database design and optimization".to_string(),
                    "Security and authentication".to_string(),
                    "Performance and scalability".to_string(),
                ],
                quality_standards: vec![
                    "RESTful API principles".to_string(),
                    "Database normalization".to_string(),
                    "Security best practices".to_string(),
                    "Error handling standards".to_string(),
                ],
                preferred_tools: vec![
                    "API documentation tools".to_string(),
                    "Database migration tools".to_string(),
                    "Testing frameworks".to_string(),
                ],
                risk_considerations: vec![
                    "Security vulnerabilities".to_string(),
                    "Performance bottlenecks".to_string(),
                    "Data consistency issues".to_string(),
                ],
                validation_approaches: vec![
                    "API testing".to_string(),
                    "Security scanning".to_string(),
                    "Load testing".to_string(),
                ],
            },
        );

        // Add more persona templates as needed...
    }

    /// Create analysis phase for systematic workflow
    fn create_analysis_phase(&self, prd_analysis: &PRDAnalysis) -> Result<WorkflowPhase> {
        let tasks = vec![
            WorkflowTask {
                id: "analysis-1".to_string(),
                name: "Requirements Analysis".to_string(),
                description: "Detailed analysis of functional and non-functional requirements".to_string(),
                acceptance_criteria: vec![
                    "All requirements documented and prioritized".to_string(),
                    "Stakeholder approval obtained".to_string(),
                    "Success metrics defined".to_string(),
                ],
                dependencies: vec![],
                complexity: Some(ComplexityEstimate {
                    level: "Medium".to_string(),
                    time_estimate: "1-2 days".to_string(),
                    confidence: 0.8,
                    factors: vec!["Requirement complexity".to_string(), "Stakeholder availability".to_string()],
                }),
                risks: vec![],
                parallelizable: false,
                estimated_hours: Some(16),
                persona: Some(ExpertPersona::Analyst),
                tools_required: vec!["Documentation tools".to_string()],
                validation_steps: vec!["Stakeholder review".to_string()],
            },
            WorkflowTask {
                id: "analysis-2".to_string(),
                name: "Technology Assessment".to_string(),
                description: "Evaluate and select appropriate technologies for implementation".to_string(),
                acceptance_criteria: vec![
                    "Technology stack documented".to_string(),
                    "Trade-offs analyzed".to_string(),
                    "Team capabilities assessed".to_string(),
                ],
                dependencies: vec!["analysis-1".to_string()],
                complexity: Some(ComplexityEstimate {
                    level: "High".to_string(),
                    time_estimate: "2-3 days".to_string(),
                    confidence: 0.7,
                    factors: vec!["Technology variety".to_string(), "Integration complexity".to_string()],
                }),
                risks: vec![],
                parallelizable: false,
                estimated_hours: Some(24),
                persona: Some(ExpertPersona::Architect),
                tools_required: vec!["Architecture tools".to_string()],
                validation_steps: vec!["Technical review".to_string()],
            },
        ];

        Ok(WorkflowPhase {
            name: "Analysis and Planning".to_string(),
            description: "Comprehensive analysis of requirements and technology assessment".to_string(),
            duration_estimate: Some("1 week".to_string()),
            tasks,
            deliverables: vec![
                "Requirements specification".to_string(),
                "Technology assessment report".to_string(),
                "Project timeline".to_string(),
            ],
            success_criteria: vec![
                "All requirements clearly defined".to_string(),
                "Technology choices validated".to_string(),
                "Team alignment achieved".to_string(),
            ],
            risks: vec![
                RiskAssessment {
                    level: RiskLevel::Medium,
                    description: "Requirements changes during analysis".to_string(),
                    probability: 0.3,
                    impact: 0.6,
                    mitigation_strategies: vec![
                        "Regular stakeholder communication".to_string(),
                        "Agile requirements management".to_string(),
                    ],
                },
            ],
        })
    }

    /// Create architecture phase for systematic workflow
    fn create_architecture_phase(
        &self,
        prd_analysis: &PRDAnalysis,
        primary_persona: Option<&ExpertPersona>,
    ) -> Result<WorkflowPhase> {
        let mut tasks = vec![
            WorkflowTask {
                id: "arch-1".to_string(),
                name: "System Architecture Design".to_string(),
                description: "Design overall system architecture and component interactions".to_string(),
                acceptance_criteria: vec![
                    "Architecture diagrams completed".to_string(),
                    "Component responsibilities defined".to_string(),
                    "Integration patterns specified".to_string(),
                ],
                dependencies: vec!["analysis-2".to_string()],
                complexity: Some(ComplexityEstimate {
                    level: "High".to_string(),
                    time_estimate: "3-5 days".to_string(),
                    confidence: 0.7,
                    factors: vec!["System complexity".to_string(), "Integration requirements".to_string()],
                }),
                risks: vec![],
                parallelizable: false,
                estimated_hours: Some(32),
                persona: Some(ExpertPersona::Architect),
                tools_required: vec!["Architecture tools".to_string(), "Diagramming tools".to_string()],
                validation_steps: vec!["Architecture review".to_string()],
            },
        ];

        // Add persona-specific architecture tasks
        match primary_persona {
            Some(ExpertPersona::Frontend) => {
                tasks.push(WorkflowTask {
                    id: "arch-2".to_string(),
                    name: "Frontend Architecture Design".to_string(),
                    description: "Design component hierarchy, state management, and routing".to_string(),
                    acceptance_criteria: vec![
                        "Component structure defined".to_string(),
                        "State management strategy selected".to_string(),
                        "Routing architecture designed".to_string(),
                    ],
                    dependencies: vec!["arch-1".to_string()],
                    complexity: Some(ComplexityEstimate {
                        level: "Medium".to_string(),
                        time_estimate: "2-3 days".to_string(),
                        confidence: 0.8,
                        factors: vec!["UI complexity".to_string()],
                    }),
                    risks: vec![],
                    parallelizable: true,
                    estimated_hours: Some(20),
                    persona: Some(ExpertPersona::Frontend),
                    tools_required: vec!["Frontend frameworks".to_string()],
                    validation_steps: vec!["UI/UX review".to_string()],
                });
            },
            Some(ExpertPersona::Backend) => {
                tasks.push(WorkflowTask {
                    id: "arch-2".to_string(),
                    name: "Backend Architecture Design".to_string(),
                    description: "Design API structure, database schema, and service layers".to_string(),
                    acceptance_criteria: vec![
                        "API specification completed".to_string(),
                        "Database schema designed".to_string(),
                        "Service boundaries defined".to_string(),
                    ],
                    dependencies: vec!["arch-1".to_string()],
                    complexity: Some(ComplexityEstimate {
                        level: "High".to_string(),
                        time_estimate: "3-4 days".to_string(),
                        confidence: 0.7,
                        factors: vec!["Data complexity".to_string(), "API requirements".to_string()],
                    }),
                    risks: vec![],
                    parallelizable: true,
                    estimated_hours: Some(28),
                    persona: Some(ExpertPersona::Backend),
                    tools_required: vec!["Database tools".to_string(), "API design tools".to_string()],
                    validation_steps: vec!["Database review".to_string(), "API review".to_string()],
                });
            },
            _ => {}
        }

        Ok(WorkflowPhase {
            name: "Architecture and Design".to_string(),
            description: "System and component architecture design".to_string(),
            duration_estimate: Some("1-2 weeks".to_string()),
            tasks,
            deliverables: vec![
                "System architecture documentation".to_string(),
                "Component specifications".to_string(),
                "Database schema".to_string(),
                "API specification".to_string(),
            ],
            success_criteria: vec![
                "Architecture approved by stakeholders".to_string(),
                "Technical feasibility validated".to_string(),
                "Implementation roadmap defined".to_string(),
            ],
            risks: vec![
                RiskAssessment {
                    level: RiskLevel::Medium,
                    description: "Architecture complexity underestimated".to_string(),
                    probability: 0.4,
                    impact: 0.7,
                    mitigation_strategies: vec![
                        "Prototype critical components".to_string(),
                        "Regular architecture reviews".to_string(),
                    ],
                },
            ],
        })
    }

    /// Create implementation phase for systematic workflow
    fn create_implementation_phase(
        &self,
        prd_analysis: &PRDAnalysis,
        primary_persona: Option<&ExpertPersona>,
    ) -> Result<WorkflowPhase> {
        let mut tasks = vec![
            WorkflowTask {
                id: "impl-1".to_string(),
                name: "Development Environment Setup".to_string(),
                description: "Set up development environment, CI/CD pipeline, and project structure".to_string(),
                acceptance_criteria: vec![
                    "Development environment configured".to_string(),
                    "CI/CD pipeline operational".to_string(),
                    "Project structure established".to_string(),
                ],
                dependencies: vec!["arch-1".to_string()],
                complexity: Some(ComplexityEstimate {
                    level: "Medium".to_string(),
                    time_estimate: "2-3 days".to_string(),
                    confidence: 0.8,
                    factors: vec!["Infrastructure complexity".to_string()],
                }),
                risks: vec![],
                parallelizable: false,
                estimated_hours: Some(20),
                persona: Some(ExpertPersona::DevOps),
                tools_required: vec!["CI/CD tools".to_string(), "Development tools".to_string()],
                validation_steps: vec!["Environment verification".to_string()],
            },
        ];

        // Add technology-specific implementation tasks based on detected stack
        for tech in &prd_analysis.technology_stack {
            match tech.as_str() {
                "Rust" => {
                    tasks.push(self.create_rust_implementation_task()?);
                },
                "React" => {
                    tasks.push(self.create_react_implementation_task()?);
                },
                "PostgreSQL" => {
                    tasks.push(self.create_database_implementation_task()?);
                },
                _ => {}
            }
        }

        // Add core feature implementation tasks
        tasks.extend(self.create_core_feature_tasks(prd_analysis)?);

        Ok(WorkflowPhase {
            name: "Core Implementation".to_string(),
            description: "Implementation of core features and functionality".to_string(),
            duration_estimate: Some("3-4 weeks".to_string()),
            tasks,
            deliverables: vec![
                "Core functionality implemented".to_string(),
                "Unit tests completed".to_string(),
                "Code documentation".to_string(),
            ],
            success_criteria: vec![
                "All core features working".to_string(),
                "Test coverage >80%".to_string(),
                "Code quality standards met".to_string(),
            ],
            risks: vec![
                RiskAssessment {
                    level: RiskLevel::High,
                    description: "Implementation complexity higher than expected".to_string(),
                    probability: 0.5,
                    impact: 0.8,
                    mitigation_strategies: vec![
                        "Break down complex tasks".to_string(),
                        "Regular progress reviews".to_string(),
                        "Technical spike investigations".to_string(),
                    ],
                },
            ],
        })
    }

    /// Create integration phase for systematic workflow
    fn create_integration_phase(&self, prd_analysis: &PRDAnalysis) -> Result<WorkflowPhase> {
        let tasks = vec![
            WorkflowTask {
                id: "int-1".to_string(),
                name: "Component Integration".to_string(),
                description: "Integrate all components and test system interactions".to_string(),
                acceptance_criteria: vec![
                    "All components integrated".to_string(),
                    "Integration tests passing".to_string(),
                    "End-to-end workflows validated".to_string(),
                ],
                dependencies: vec!["impl-core".to_string()],
                complexity: Some(ComplexityEstimate {
                    level: "High".to_string(),
                    time_estimate: "1-2 weeks".to_string(),
                    confidence: 0.6,
                    factors: vec!["Integration complexity".to_string(), "Component interdependencies".to_string()],
                }),
                risks: vec![],
                parallelizable: false,
                estimated_hours: Some(60),
                persona: Some(ExpertPersona::QA),
                tools_required: vec!["Testing frameworks".to_string()],
                validation_steps: vec!["Integration testing".to_string()],
            },
            WorkflowTask {
                id: "int-2".to_string(),
                name: "Performance Testing".to_string(),
                description: "Conduct performance testing and optimization".to_string(),
                acceptance_criteria: vec![
                    "Performance benchmarks met".to_string(),
                    "Load testing completed".to_string(),
                    "Optimization implemented".to_string(),
                ],
                dependencies: vec!["int-1".to_string()],
                complexity: Some(ComplexityEstimate {
                    level: "Medium".to_string(),
                    time_estimate: "3-5 days".to_string(),
                    confidence: 0.7,
                    factors: vec!["Performance requirements".to_string()],
                }),
                risks: vec![],
                parallelizable: true,
                estimated_hours: Some(32),
                persona: Some(ExpertPersona::Performance),
                tools_required: vec!["Performance testing tools".to_string()],
                validation_steps: vec!["Performance validation".to_string()],
            },
        ];

        Ok(WorkflowPhase {
            name: "Integration and Testing".to_string(),
            description: "System integration, testing, and performance validation".to_string(),
            duration_estimate: Some("2 weeks".to_string()),
            tasks,
            deliverables: vec![
                "Integrated system".to_string(),
                "Test reports".to_string(),
                "Performance benchmarks".to_string(),
            ],
            success_criteria: vec![
                "All integration tests passing".to_string(),
                "Performance targets met".to_string(),
                "System ready for deployment".to_string(),
            ],
            risks: vec![
                RiskAssessment {
                    level: RiskLevel::Medium,
                    description: "Integration issues discovered late".to_string(),
                    probability: 0.4,
                    impact: 0.6,
                    mitigation_strategies: vec![
                        "Continuous integration".to_string(),
                        "Early integration testing".to_string(),
                    ],
                },
            ],
        })
    }

    /// Create deployment phase for systematic workflow
    fn create_deployment_phase(&self, prd_analysis: &PRDAnalysis) -> Result<WorkflowPhase> {
        let tasks = vec![
            WorkflowTask {
                id: "deploy-1".to_string(),
                name: "Production Environment Setup".to_string(),
                description: "Set up production environment and infrastructure".to_string(),
                acceptance_criteria: vec![
                    "Production environment configured".to_string(),
                    "Monitoring and logging set up".to_string(),
                    "Security measures implemented".to_string(),
                ],
                dependencies: vec!["int-2".to_string()],
                complexity: Some(ComplexityEstimate {
                    level: "High".to_string(),
                    time_estimate: "1 week".to_string(),
                    confidence: 0.7,
                    factors: vec!["Infrastructure complexity".to_string(), "Security requirements".to_string()],
                }),
                risks: vec![],
                parallelizable: false,
                estimated_hours: Some(40),
                persona: Some(ExpertPersona::DevOps),
                tools_required: vec!["Infrastructure tools".to_string(), "Monitoring tools".to_string()],
                validation_steps: vec!["Infrastructure validation".to_string()],
            },
            WorkflowTask {
                id: "deploy-2".to_string(),
                name: "Production Deployment".to_string(),
                description: "Deploy system to production and conduct go-live activities".to_string(),
                acceptance_criteria: vec![
                    "System successfully deployed".to_string(),
                    "All health checks passing".to_string(),
                    "Rollback plan validated".to_string(),
                ],
                dependencies: vec!["deploy-1".to_string()],
                complexity: Some(ComplexityEstimate {
                    level: "Medium".to_string(),
                    time_estimate: "2-3 days".to_string(),
                    confidence: 0.8,
                    factors: vec!["Deployment complexity".to_string()],
                }),
                risks: vec![],
                parallelizable: false,
                estimated_hours: Some(24),
                persona: Some(ExpertPersona::DevOps),
                tools_required: vec!["Deployment tools".to_string()],
                validation_steps: vec!["Production validation".to_string()],
            },
        ];

        Ok(WorkflowPhase {
            name: "Deployment and Monitoring".to_string(),
            description: "Production deployment and monitoring setup".to_string(),
            duration_estimate: Some("1 week".to_string()),
            tasks,
            deliverables: vec![
                "Production deployment".to_string(),
                "Monitoring dashboard".to_string(),
                "Operations documentation".to_string(),
            ],
            success_criteria: vec![
                "System operational in production".to_string(),
                "Monitoring and alerting active".to_string(),
                "Team trained on operations".to_string(),
            ],
            risks: vec![
                RiskAssessment {
                    level: RiskLevel::High,
                    description: "Production deployment issues".to_string(),
                    probability: 0.3,
                    impact: 0.9,
                    mitigation_strategies: vec![
                        "Blue-green deployment".to_string(),
                        "Comprehensive rollback plan".to_string(),
                        "Staging environment validation".to_string(),
                    ],
                },
            ],
        })
    }

    // Additional helper methods for creating other workflow phases and components
    // ... (truncated for brevity, would include implementations for other phases)

    /// Create Rust-specific implementation task
    fn create_rust_implementation_task(&self) -> Result<WorkflowTask> {
        Ok(WorkflowTask {
            id: "impl-rust".to_string(),
            name: "Rust Service Implementation".to_string(),
            description: "Implement core services using Rust with async patterns".to_string(),
            acceptance_criteria: vec![
                "Service interfaces implemented".to_string(),
                "Error handling follows SchedulerResult pattern".to_string(),
                "Async patterns used correctly".to_string(),
                "Unit tests for all services".to_string(),
            ],
            dependencies: vec!["impl-1".to_string()],
            complexity: Some(ComplexityEstimate {
                level: "High".to_string(),
                time_estimate: "1-2 weeks".to_string(),
                confidence: 0.7,
                factors: vec!["Rust complexity".to_string(), "Async programming".to_string()],
            }),
            risks: vec![
                RiskAssessment {
                    level: RiskLevel::Medium,
                    description: "Rust learning curve for team".to_string(),
                    probability: 0.6,
                    impact: 0.5,
                    mitigation_strategies: vec![
                        "Rust training sessions".to_string(),
                        "Pair programming".to_string(),
                        "Code reviews".to_string(),
                    ],
                },
            ],
            parallelizable: true,
            estimated_hours: Some(80),
            persona: Some(ExpertPersona::Backend),
            tools_required: vec!["Rust toolchain".to_string(), "Cargo".to_string()],
            validation_steps: vec!["Code review".to_string(), "Unit tests".to_string()],
        })
    }

    /// Create React-specific implementation task
    fn create_react_implementation_task(&self) -> Result<WorkflowTask> {
        Ok(WorkflowTask {
            id: "impl-react".to_string(),
            name: "React Frontend Implementation".to_string(),
            description: "Implement frontend components using React with TypeScript".to_string(),
            acceptance_criteria: vec![
                "All UI components implemented".to_string(),
                "TypeScript types defined".to_string(),
                "Responsive design implemented".to_string(),
                "Component tests written".to_string(),
            ],
            dependencies: vec!["impl-1".to_string()],
            complexity: Some(ComplexityEstimate {
                level: "Medium".to_string(),
                time_estimate: "1-2 weeks".to_string(),
                confidence: 0.8,
                factors: vec!["UI complexity".to_string(), "Component interactions".to_string()],
            }),
            risks: vec![],
            parallelizable: true,
            estimated_hours: Some(60),
            persona: Some(ExpertPersona::Frontend),
            tools_required: vec!["React".to_string(), "TypeScript".to_string()],
            validation_steps: vec!["Component tests".to_string(), "UI review".to_string()],
        })
    }

    /// Create database-specific implementation task
    fn create_database_implementation_task(&self) -> Result<WorkflowTask> {
        Ok(WorkflowTask {
            id: "impl-db".to_string(),
            name: "Database Implementation".to_string(),
            description: "Implement database schema, migrations, and repository patterns".to_string(),
            acceptance_criteria: vec![
                "Database schema created".to_string(),
                "Migration scripts implemented".to_string(),
                "Repository patterns implemented".to_string(),
                "Database tests written".to_string(),
            ],
            dependencies: vec!["impl-1".to_string()],
            complexity: Some(ComplexityEstimate {
                level: "Medium".to_string(),
                time_estimate: "1 week".to_string(),
                confidence: 0.8,
                factors: vec!["Schema complexity".to_string(), "Data relationships".to_string()],
            }),
            risks: vec![],
            parallelizable: true,
            estimated_hours: Some(40),
            persona: Some(ExpertPersona::Backend),
            tools_required: vec!["PostgreSQL".to_string(), "SQLx".to_string()],
            validation_steps: vec!["Database tests".to_string(), "Migration tests".to_string()],
        })
    }

    /// Create core feature implementation tasks based on requirements
    fn create_core_feature_tasks(&self, prd_analysis: &PRDAnalysis) -> Result<Vec<WorkflowTask>> {
        let mut tasks = Vec::new();
        
        // Create task for each requirement
        for (index, requirement) in prd_analysis.requirements.iter().enumerate() {
            let task_id = format!("impl-feature-{}", index + 1);
            let task = WorkflowTask {
                id: task_id,
                name: format!("Implement: {}", requirement),
                description: format!("Implement the requirement: {}", requirement),
                acceptance_criteria: vec![
                    "Feature implemented according to specification".to_string(),
                    "Unit tests written and passing".to_string(),
                    "Integration with existing components verified".to_string(),
                ],
                dependencies: vec!["impl-1".to_string()],
                complexity: Some(ComplexityEstimate {
                    level: "Medium".to_string(),
                    time_estimate: "3-5 days".to_string(),
                    confidence: 0.7,
                    factors: vec!["Feature complexity".to_string()],
                }),
                risks: vec![],
                parallelizable: true,
                estimated_hours: Some(32),
                persona: None, // Will be determined by content
                tools_required: vec!["Development tools".to_string()],
                validation_steps: vec!["Feature testing".to_string()],
            };
            tasks.push(task);
        }

        // Add a core integration task
        tasks.push(WorkflowTask {
            id: "impl-core".to_string(),
            name: "Core Feature Integration".to_string(),
            description: "Integrate all core features and ensure they work together".to_string(),
            acceptance_criteria: vec![
                "All core features integrated".to_string(),
                "Feature interactions tested".to_string(),
                "End-to-end workflows validated".to_string(),
            ],
            dependencies: tasks.iter().map(|t| t.id.clone()).collect(),
            complexity: Some(ComplexityEstimate {
                level: "High".to_string(),
                time_estimate: "1 week".to_string(),
                confidence: 0.6,
                factors: vec!["Integration complexity".to_string()],
            }),
            risks: vec![],
            parallelizable: false,
            estimated_hours: Some(40),
            persona: Some(ExpertPersona::Architect),
            tools_required: vec!["Integration tools".to_string()],
            validation_steps: vec!["Integration testing".to_string()],
        });

        Ok(tasks)
    }

    /// Identify overall project dependencies
    fn identify_overall_dependencies(&self, prd_analysis: &PRDAnalysis) -> Result<Vec<Dependency>> {
        let mut dependencies = Vec::new();

        // Technology dependencies
        for tech in &prd_analysis.technology_stack {
            dependencies.push(Dependency {
                name: tech.clone(),
                dependency_type: DependencyType::Technical,
                description: format!("Required technology: {}", tech),
                criticality: RiskLevel::High,
                estimated_effort: Some("Setup and configuration".to_string()),
            });
        }

        // Infrastructure dependencies
        dependencies.push(Dependency {
            name: "Development Environment".to_string(),
            dependency_type: DependencyType::Infrastructure,
            description: "Development environment setup and configuration".to_string(),
            criticality: RiskLevel::High,
            estimated_effort: Some("2-3 days".to_string()),
        });

        dependencies.push(Dependency {
            name: "CI/CD Pipeline".to_string(),
            dependency_type: DependencyType::Infrastructure,
            description: "Continuous integration and deployment pipeline".to_string(),
            criticality: RiskLevel::Medium,
            estimated_effort: Some("1-2 days".to_string()),
        });

        // Team dependencies
        dependencies.push(Dependency {
            name: "Team Training".to_string(),
            dependency_type: DependencyType::Team,
            description: "Team training on technologies and processes".to_string(),
            criticality: RiskLevel::Medium,
            estimated_effort: Some("Ongoing".to_string()),
        });

        Ok(dependencies)
    }

    /// Identify MVP-specific dependencies (reduced scope)
    fn identify_mvp_dependencies(&self, prd_analysis: &PRDAnalysis) -> Result<Vec<Dependency>> {
        let mut dependencies = self.identify_overall_dependencies(prd_analysis)?;
        
        // Filter to only critical dependencies for MVP
        dependencies.retain(|dep| dep.criticality == RiskLevel::High || dep.criticality == RiskLevel::Critical);
        
        Ok(dependencies)
    }

    /// Identify tasks that can run in parallel
    fn identify_parallel_streams(&self, _prd_analysis: &PRDAnalysis) -> Result<Vec<Vec<String>>> {
        let parallel_streams = vec![
            vec!["arch-2".to_string(), "impl-rust".to_string(), "impl-react".to_string()],
            vec!["impl-feature-1".to_string(), "impl-feature-2".to_string(), "impl-feature-3".to_string()],
            vec!["int-2".to_string(), "security-audit".to_string()],
        ];
        
        Ok(parallel_streams)
    }

    /// Create milestones for systematic workflow
    fn create_systematic_milestones(&self) -> Result<Vec<Milestone>> {
        let milestones = vec![
            Milestone {
                name: "Requirements Finalized".to_string(),
                description: "All requirements analyzed and approved".to_string(),
                completion_criteria: vec![
                    "Requirements specification approved".to_string(),
                    "Technology stack selected".to_string(),
                ],
                target_phase: "Analysis and Planning".to_string(),
                dependencies: vec!["analysis-1".to_string(), "analysis-2".to_string()],
            },
            Milestone {
                name: "Architecture Complete".to_string(),
                description: "System architecture designed and approved".to_string(),
                completion_criteria: vec![
                    "Architecture documentation complete".to_string(),
                    "Component interfaces defined".to_string(),
                ],
                target_phase: "Architecture and Design".to_string(),
                dependencies: vec!["arch-1".to_string()],
            },
            Milestone {
                name: "Core Features Complete".to_string(),
                description: "All core features implemented and tested".to_string(),
                completion_criteria: vec![
                    "All core features working".to_string(),
                    "Unit tests passing".to_string(),
                ],
                target_phase: "Core Implementation".to_string(),
                dependencies: vec!["impl-core".to_string()],
            },
            Milestone {
                name: "System Ready for Production".to_string(),
                description: "System integrated, tested, and ready for deployment".to_string(),
                completion_criteria: vec![
                    "Integration tests passing".to_string(),
                    "Performance requirements met".to_string(),
                    "Security validation complete".to_string(),
                ],
                target_phase: "Integration and Testing".to_string(),
                dependencies: vec!["int-1".to_string(), "int-2".to_string()],
            },
        ];
        
        Ok(milestones)
    }

    /// Create milestones for agile workflow
    fn create_agile_milestones(&self) -> Result<Vec<Milestone>> {
        let milestones = vec![
            Milestone {
                name: "Sprint 0 Complete".to_string(),
                description: "Project setup and initial planning complete".to_string(),
                completion_criteria: vec![
                    "Development environment ready".to_string(),
                    "Initial backlog created".to_string(),
                ],
                target_phase: "Sprint 0".to_string(),
                dependencies: vec!["sprint0-setup".to_string()],
            },
            Milestone {
                name: "MVP Features Complete".to_string(),
                description: "Core MVP features implemented".to_string(),
                completion_criteria: vec![
                    "MVP functionality working".to_string(),
                    "Basic testing complete".to_string(),
                ],
                target_phase: "Core Features Sprint".to_string(),
                dependencies: vec!["core-features".to_string()],
            },
            Milestone {
                name: "Production Ready".to_string(),
                description: "System polished and ready for production".to_string(),
                completion_criteria: vec![
                    "All features polished".to_string(),
                    "Production deployment successful".to_string(),
                ],
                target_phase: "Enhancement Sprint".to_string(),
                dependencies: vec!["enhancement-complete".to_string()],
            },
        ];
        
        Ok(milestones)
    }

    /// Create milestones for MVP workflow
    fn create_mvp_milestones(&self) -> Result<Vec<Milestone>> {
        let milestones = vec![
            Milestone {
                name: "MVP Scope Defined".to_string(),
                description: "MVP features clearly defined and prioritized".to_string(),
                completion_criteria: vec![
                    "MVP features list finalized".to_string(),
                    "Success metrics defined".to_string(),
                ],
                target_phase: "MVP Definition".to_string(),
                dependencies: vec!["mvp-definition".to_string()],
            },
            Milestone {
                name: "MVP Implementation Complete".to_string(),
                description: "Core MVP functionality implemented".to_string(),
                completion_criteria: vec![
                    "All MVP features working".to_string(),
                    "Basic validation complete".to_string(),
                ],
                target_phase: "MVP Implementation".to_string(),
                dependencies: vec!["mvp-implementation".to_string()],
            },
            Milestone {
                name: "MVP Launch Ready".to_string(),
                description: "MVP validated and ready for user feedback".to_string(),
                completion_criteria: vec![
                    "User acceptance testing complete".to_string(),
                    "Feedback collection system ready".to_string(),
                ],
                target_phase: "MVP Validation".to_string(),
                dependencies: vec!["mvp-validation".to_string()],
            },
        ];
        
        Ok(milestones)
    }

    // Additional phase creation methods for Agile and MVP strategies
    // ... (placeholder methods that would be implemented similarly)

    fn create_sprint_zero_phase(&self, _prd_analysis: &PRDAnalysis) -> Result<WorkflowPhase> {
        // Implementation for Sprint 0 setup phase
        Ok(WorkflowPhase {
            name: "Sprint 0: Setup".to_string(),
            description: "Initial project setup and planning".to_string(),
            duration_estimate: Some("1 week".to_string()),
            tasks: vec![], // Would be populated with sprint 0 tasks
            deliverables: vec![],
            success_criteria: vec![],
            risks: vec![],
        })
    }

    fn create_core_features_sprint(&self, _prd_analysis: &PRDAnalysis, _persona: Option<&ExpertPersona>) -> Result<WorkflowPhase> {
        // Implementation for core features sprint
        Ok(WorkflowPhase {
            name: "Core Features Sprint".to_string(),
            description: "Implementation of core features".to_string(),
            duration_estimate: Some("4-6 weeks".to_string()),
            tasks: vec![], // Would be populated with core feature tasks
            deliverables: vec![],
            success_criteria: vec![],
            risks: vec![],
        })
    }

    fn create_enhancement_sprint(&self, _prd_analysis: &PRDAnalysis, _persona: Option<&ExpertPersona>) -> Result<WorkflowPhase> {
        // Implementation for enhancement sprint
        Ok(WorkflowPhase {
            name: "Enhancement Sprint".to_string(),
            description: "Feature enhancement and polishing".to_string(),
            duration_estimate: Some("2-3 weeks".to_string()),
            tasks: vec![], // Would be populated with enhancement tasks
            deliverables: vec![],
            success_criteria: vec![],
            risks: vec![],
        })
    }

    fn create_mvp_definition_phase(&self, _prd_analysis: &PRDAnalysis) -> Result<WorkflowPhase> {
        // Implementation for MVP definition phase
        Ok(WorkflowPhase {
            name: "MVP Definition".to_string(),
            description: "Define MVP scope and features".to_string(),
            duration_estimate: Some("2-3 days".to_string()),
            tasks: vec![], // Would be populated with MVP definition tasks
            deliverables: vec![],
            success_criteria: vec![],
            risks: vec![],
        })
    }

    fn create_mvp_implementation_phase(&self, _prd_analysis: &PRDAnalysis, _persona: Option<&ExpertPersona>) -> Result<WorkflowPhase> {
        // Implementation for MVP implementation phase
        Ok(WorkflowPhase {
            name: "MVP Implementation".to_string(),
            description: "Implement core MVP functionality".to_string(),
            duration_estimate: Some("2-3 weeks".to_string()),
            tasks: vec![], // Would be populated with MVP implementation tasks
            deliverables: vec![],
            success_criteria: vec![],
            risks: vec![],
        })
    }

    fn create_mvp_validation_phase(&self, _prd_analysis: &PRDAnalysis) -> Result<WorkflowPhase> {
        // Implementation for MVP validation phase
        Ok(WorkflowPhase {
            name: "MVP Validation".to_string(),
            description: "Validate MVP and collect feedback".to_string(),
            duration_estimate: Some("1 week".to_string()),
            tasks: vec![], // Would be populated with MVP validation tasks
            deliverables: vec![],
            success_criteria: vec![],
            risks: vec![],
        })
    }

    /// Format workflow as roadmap
    pub fn format_as_roadmap(&self, workflow: &WorkflowSpecification) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("# {}\n\n", workflow.title));
        output.push_str(&format!("{}\n\n", workflow.description));
        
        if let Some(duration) = &workflow.total_estimated_duration {
            output.push_str(&format!("**Total Estimated Duration:** {}\n\n", duration));
        }

        for (index, phase) in workflow.phases.iter().enumerate() {
            output.push_str(&format!("## Phase {}: {} ({})\n\n", 
                index + 1, 
                phase.name, 
                phase.duration_estimate.as_ref().unwrap_or(&"TBD".to_string())
            ));
            
            for task in &phase.tasks {
                let status = if task.parallelizable { "🔄" } else { "📋" };
                let hours = task.estimated_hours.map(|h| format!(" ({}h)", h)).unwrap_or_default();
                output.push_str(&format!("- [ ] {}{}{}\n", status, task.name, hours));
            }
            
            output.push_str("\n**Deliverables:**\n");
            for deliverable in &phase.deliverables {
                output.push_str(&format!("- {}\n", deliverable));
            }
            output.push_str("\n");
        }

        if !workflow.milestones.is_empty() {
            output.push_str("## Key Milestones\n\n");
            for milestone in &workflow.milestones {
                output.push_str(&format!("### {} ({})\n", milestone.name, milestone.target_phase));
                output.push_str(&format!("{}\n\n", milestone.description));
            }
        }

        output
    }

    /// Format workflow as detailed tasks
    pub fn format_as_tasks(&self, workflow: &WorkflowSpecification) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("# Implementation Tasks: {}\n\n", workflow.title));
        
        for phase in &workflow.phases {
            output.push_str(&format!("## Epic: {}\n\n", phase.name));
            
            for task in &phase.tasks {
                output.push_str(&format!("### Story: {}\n", task.name));
                output.push_str(&format!("{}\n\n", task.description));
                
                if !task.acceptance_criteria.is_empty() {
                    output.push_str("**Acceptance Criteria:**\n");
                    for criteria in &task.acceptance_criteria {
                        output.push_str(&format!("- [ ] {}\n", criteria));
                    }
                    output.push_str("\n");
                }
                
                if let Some(complexity) = &task.complexity {
                    output.push_str(&format!("**Complexity:** {} ({})\n", 
                        complexity.level, complexity.time_estimate));
                    output.push_str(&format!("**Confidence:** {:.0}%\n\n", complexity.confidence * 100.0));
                }
            }
        }

        output
    }

    /// Format workflow with full details
    pub fn format_as_detailed(&self, workflow: &WorkflowSpecification) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("# Detailed Implementation Workflow: {}\n\n", workflow.title));
        output.push_str(&format!("{}\n\n", workflow.description));
        
        output.push_str(&format!("**Strategy:** {:?}\n", workflow.strategy));
        if let Some(persona) = &workflow.primary_persona {
            output.push_str(&format!("**Primary Persona:** {:?}\n", persona));
        }
        if let Some(duration) = &workflow.total_estimated_duration {
            output.push_str(&format!("**Total Duration:** {}\n", duration));
        }
        output.push_str("\n");

        for phase in &workflow.phases {
            output.push_str(&format!("## Phase: {}\n\n", phase.name));
            output.push_str(&format!("{}\n\n", phase.description));
            
            if let Some(duration) = &phase.duration_estimate {
                output.push_str(&format!("**Duration:** {}\n\n", duration));
            }

            for task in &phase.tasks {
                output.push_str(&format!("### Task: {}\n", task.name));
                if let Some(persona) = &task.persona {
                    output.push_str(&format!("**Persona:** {:?}\n", persona));
                }
                if let Some(hours) = task.estimated_hours {
                    output.push_str(&format!("**Estimated Time:** {} hours\n", hours));
                }
                if !task.dependencies.is_empty() {
                    output.push_str(&format!("**Dependencies:** {}\n", task.dependencies.join(", ")));
                }
                output.push_str("\n");
                
                output.push_str(&format!("{}\n\n", task.description));
                
                if !task.acceptance_criteria.is_empty() {
                    output.push_str("**Acceptance Criteria:**\n");
                    for criteria in &task.acceptance_criteria {
                        output.push_str(&format!("- [ ] {}\n", criteria));
                    }
                    output.push_str("\n");
                }
                
                if let Some(complexity) = &task.complexity {
                    output.push_str("**Complexity Analysis:**\n");
                    output.push_str(&format!("- Level: {}\n", complexity.level));
                    output.push_str(&format!("- Time Estimate: {}\n", complexity.time_estimate));
                    output.push_str(&format!("- Confidence: {:.0}%\n", complexity.confidence * 100.0));
                    if !complexity.factors.is_empty() {
                        output.push_str("- Factors: ");
                        output.push_str(&complexity.factors.join(", "));
                        output.push_str("\n");
                    }
                    output.push_str("\n");
                }
                
                if !task.validation_steps.is_empty() {
                    output.push_str("**Validation Steps:**\n");
                    for step in &task.validation_steps {
                        output.push_str(&format!("- [ ] {}\n", step));
                    }
                    output.push_str("\n");
                }
            }
        }

        if !workflow.overall_dependencies.is_empty() {
            output.push_str("## Dependencies\n\n");
            for dep in &workflow.overall_dependencies {
                output.push_str(&format!("### {} ({:?})\n", dep.name, dep.dependency_type));
                output.push_str(&format!("{}\n", dep.description));
                output.push_str(&format!("**Criticality:** {:?}\n", dep.criticality));
                if let Some(effort) = &dep.estimated_effort {
                    output.push_str(&format!("**Effort:** {}\n", effort));
                }
                output.push_str("\n");
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_generator_creation() {
        let config = WorkflowConfig::default();
        let generator = WorkflowGenerator::new(config);
        
        // Test that generator is created with persona templates
        assert!(!generator.persona_templates.is_empty());
        assert!(generator.persona_templates.contains_key(&ExpertPersona::Architect));
        assert!(generator.persona_templates.contains_key(&ExpertPersona::Frontend));
        assert!(generator.persona_templates.contains_key(&ExpertPersona::Backend));
    }

    #[tokio::test]
    async fn test_feature_description_analysis() {
        let config = WorkflowConfig::default();
        let generator = WorkflowGenerator::new(config);
        
        let description = "Implement a user authentication API with JWT tokens and password hashing";
        let analysis = generator.analyze_feature_description(description).await.unwrap();
        
        assert!(!analysis.title.is_empty());
        assert!(!analysis.objectives.is_empty());
        assert!(!analysis.requirements.is_empty());
        assert!(!analysis.domain_indicators.is_empty());
    }

    #[tokio::test]
    async fn test_persona_detection() {
        let config = WorkflowConfig::default();
        let generator = WorkflowGenerator::new(config);
        
        let frontend_analysis = PRDAnalysis {
            title: "React Dashboard Component".to_string(),
            objectives: vec!["Create responsive UI dashboard".to_string()],
            requirements: vec!["React components".to_string(), "responsive design".to_string()],
            acceptance_criteria: vec![],
            constraints: vec![],
            stakeholders: vec![],
            success_metrics: vec![],
            domain_indicators: vec![ExpertPersona::Frontend],
            complexity_indicators: vec![],
            technology_stack: vec!["React".to_string()],
        };
        
        let persona = generator.detect_primary_persona(&frontend_analysis);
        assert_eq!(persona, Some(ExpertPersona::Frontend));
    }

    #[tokio::test]
    async fn test_workflow_generation() {
        let config = WorkflowConfig::default();
        let generator = WorkflowGenerator::new(config);
        
        let description = "Build a task scheduler API with database integration";
        let workflow = generator.generate_workflow(description).await.unwrap();
        
        assert!(!workflow.title.is_empty());
        assert!(!workflow.phases.is_empty());
        assert_eq!(workflow.strategy, WorkflowStrategy::Systematic);
        
        // Check that phases contain tasks
        for phase in &workflow.phases {
            assert!(!phase.tasks.is_empty());
            assert!(!phase.deliverables.is_empty());
            assert!(!phase.success_criteria.is_empty());
        }
    }

    #[tokio::test]
    async fn test_output_formatting() {
        let config = WorkflowConfig::default();
        let generator = WorkflowGenerator::new(config);
        
        let description = "Simple API endpoint";
        let workflow = generator.generate_workflow(description).await.unwrap();
        
        let roadmap = generator.format_as_roadmap(&workflow);
        assert!(roadmap.contains("# Systematic Implementation"));
        assert!(roadmap.contains("## Phase"));
        
        let tasks = generator.format_as_tasks(&workflow);
        assert!(tasks.contains("# Implementation Tasks"));
        assert!(tasks.contains("## Epic:"));
        
        let detailed = generator.format_as_detailed(&workflow);
        assert!(detailed.contains("# Detailed Implementation Workflow"));
        assert!(detailed.contains("**Strategy:**"));
    }
}