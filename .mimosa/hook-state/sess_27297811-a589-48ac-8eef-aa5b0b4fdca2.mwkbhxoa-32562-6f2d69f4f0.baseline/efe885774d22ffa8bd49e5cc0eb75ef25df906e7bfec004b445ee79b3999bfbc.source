//! OpenTofu (Terraform open-source fork) adaptation executor: renders a
//! per-request workspace (main.tf + terraform.tfvars.json), runs
//! init/apply as subprocesses with credentials injected through the
//! environment, and parses `tofu output -json` for CMDB write-back.
//!
//! Templates are curated per cloud; the `demo` provider (hashicorp/null)
//! exists so the provision pipeline can be exercised end-to-end without
//! cloud credentials. Design doc: docs/cmdb-roadmap.md.

use std::{collections::BTreeMap, path::Path, path::PathBuf, process::Stdio, time::Duration};

use serde_json::{Map, Value, json};

pub const DEMO_PROVIDER: &str = "demo";

#[derive(Debug)]
pub struct TofuExecutor {
    binary: String,
    workspace_root: PathBuf,
    timeout: Duration,
}

/// Result of one tofu invocation (or an availability probe).
#[derive(Debug, Clone)]
pub struct TofuRun {
    pub success: bool,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
}

impl TofuRun {
    pub fn combined_log(&self) -> String {
        format!(
            "$ {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.command, self.stdout, self.stderr
        )
    }
}

#[derive(Debug, Clone)]
pub struct CloudTarget {
    /// Cloud category from the ticket, e.g. aliyun / tencent / aws / demo.
    pub cloud_category: String,
    /// Provider source block, e.g. "aliyun/alicloud".
    pub provider_source: String,
    pub region: String,
    pub access_key_id: String,
    pub access_key_secret: String,
}

impl Default for TofuExecutor {
    fn default() -> Self {
        Self::from_env()
    }
}

impl TofuExecutor {
    /// TOFU_BINARY (default "tofu"), TOFU_WORKSPACE_ROOT (default
    /// /tmp/rustset-tofu), TOFU_TIMEOUT_SECONDS (default 180).
    pub fn from_env() -> Self {
        let binary = std::env::var("TOFU_BINARY").unwrap_or_else(|_| "tofu".into());
        let workspace_root = std::env::var("TOFU_WORKSPACE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("rustset-tofu"));
        let timeout = Duration::from_secs(
            std::env::var("TOFU_TIMEOUT_SECONDS")
                .ok()
                .and_then(|value| value.parse().ok())
                .filter(|seconds| *seconds > 0)
                .unwrap_or(180),
        );
        Self {
            binary,
            workspace_root,
            timeout,
        }
    }

    /// True when the configured binary executes (`tofu version`). Runs in
    /// the OS temp dir because the workspace root may not exist yet and a
    /// failed -chdir would masquerade as a missing binary.
    pub async fn available(&self) -> bool {
        std::fs::create_dir_all(&self.workspace_root).ok();
        self.run(&["version"], &[], &self.workspace_root)
            .await
            .success
    }

    /// Create (reuse) an isolated workspace directory for a request.
    pub fn ensure_workspace(&self, name: &str) -> Result<PathBuf, String> {
        let safe: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        if safe.trim_matches('-').is_empty() {
            return Err("workspace name must contain alphanumeric characters".into());
        }
        let path = self.workspace_root.join(safe);
        std::fs::create_dir_all(&path).map_err(|error| format!("create workspace: {error}"))?;
        Ok(path)
    }

    pub fn write_file(workspace: &Path, name: &str, content: &str) -> Result<(), String> {
        std::fs::write(workspace.join(name), content)
            .map_err(|error| format!("write {name}: {error}"))
    }

    pub fn read_file(workspace: &Path, name: &str) -> Result<String, String> {
        std::fs::read_to_string(workspace.join(name))
            .map_err(|error| format!("read {name}: {error}"))
    }

    /// Run the binary with `-chdir=<workspace>` prepended. Environment
    /// variables are passed only to the child.
    pub async fn run(
        &self,
        args: &[&str],
        env: &[(String, String)],
        workspace: &Path,
    ) -> TofuRun {
        let chdir = format!("-chdir={}", workspace.display());
        let mut command = tokio::process::Command::new(&self.binary);
        command
            .arg(&chdir)
            .args(args)
            .env_clear()
            .envs(env.iter().cloned())
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            // Registry access often needs the host's proxy settings.
            .env(
                "http_proxy",
                std::env::var("http_proxy").unwrap_or_default(),
            )
            .env(
                "https_proxy",
                std::env::var("https_proxy").unwrap_or_default(),
            )
            .env(
                "HTTP_PROXY",
                std::env::var("HTTP_PROXY").unwrap_or_default(),
            )
            .env(
                "HTTPS_PROXY",
                std::env::var("HTTPS_PROXY").unwrap_or_default(),
            )
            .env(
                "no_proxy",
                std::env::var("no_proxy")
                    .or_else(|_| std::env::var("NO_PROXY"))
                    .unwrap_or_default(),
            )
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let rendered = format!("{:?} {} {:?}", self.binary, chdir, args);
        match tokio::time::timeout(self.timeout, command.output()).await {
            Ok(Ok(output)) => TofuRun {
                success: output.status.success(),
                command: rendered,
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            },
            Ok(Err(error)) => TofuRun {
                success: false,
                command: rendered,
                stdout: String::new(),
                stderr: format!("spawn tofu: {error}"),
            },
            Err(_) => TofuRun {
                success: false,
                command: rendered,
                stdout: String::new(),
                stderr: format!("tofu timed out after {}s", self.timeout.as_secs()),
            },
        }
    }

    pub async fn init(&self, workspace: &Path) -> TofuRun {
        self.run(&["init", "-input=false", "-no-color"], &[], workspace)
            .await
    }

    pub async fn apply(&self, env: &[(String, String)], workspace: &Path) -> TofuRun {
        self.run(
            &["apply", "-auto-approve", "-input=false", "-no-color"],
            env,
            workspace,
        )
        .await
    }

    /// `tofu output -json` -> flat {name: value}; errors must block delivery.
    pub async fn outputs(&self, workspace: &Path) -> Result<BTreeMap<String, Value>, String> {
        let run = self
            .run(&["output", "-json", "-no-color"], &[], workspace)
            .await;
        if !run.success {
            return Err(
                "tofu output failed; resources may already exist, retain the workspace".into(),
            );
        }
        let value: Value =
            serde_json::from_str(&run.stdout).map_err(|_| "invalid tofu output JSON")?;
        if !value.is_object() || value.as_object().is_none_or(|v| v.is_empty()) {
            return Err("tofu returned no resource outputs".into());
        }
        let outputs = parse_outputs(&run.stdout);
        if outputs.is_empty() {
            return Err("tofu returned malformed resource outputs".into());
        }
        Ok(outputs)
    }
}

/// Map a cloud category to its tofu provider source; unknown categories
/// have no curated template yet.
pub fn provider_source(cloud_category: &str) -> Option<&'static str> {
    match cloud_category {
        DEMO_PROVIDER => Some("hashicorp/null"),
        "aliyun" | "alicloud" => Some("aliyun/alicloud"),
        "tencent" | "tencentcloud" => Some("tencentcloudstack/tencentcloud"),
        "huawei" | "huaweicloud" => Some("huaweicloud/huaweicloud"),
        _ => None,
    }
}

/// Credential environment variables per cloud category.
pub fn credential_env(target: &CloudTarget) -> Vec<(String, String)> {
    let mut env = vec![(String::new(), String::new())];
    env.clear();
    match target.cloud_category.as_str() {
        "aliyun" | "alicloud" => {
            env.push(("ALICLOUD_ACCESS_KEY".into(), target.access_key_id.clone()));
            env.push((
                "ALICLOUD_SECRET_KEY".into(),
                target.access_key_secret.clone(),
            ));
            env.push(("ALICLOUD_REGION".into(), target.region.clone()));
        }
        "tencent" | "tencentcloud" => {
            env.push((
                "TENCENTCLOUD_SECRET_ID".into(),
                target.access_key_id.clone(),
            ));
            env.push((
                "TENCENTCLOUD_SECRET_KEY".into(),
                target.access_key_secret.clone(),
            ));
            env.push(("TENCENTCLOUD_REGION".into(), target.region.clone()));
        }
        "huawei" | "huaweicloud" => {
            env.push(("HW_ACCESS_KEY".into(), target.access_key_id.clone()));
            env.push(("HW_SECRET_KEY".into(), target.access_key_secret.clone()));
            env.push(("HW_REGION_NAME".into(), target.region.clone()));
        }
        _ => {}
    }
    env
}

/// Render main.tf for a resource request. The demo template keeps the
/// pipeline honest end-to-end without cloud access; the aliyun template is
/// the reference for real provisioning.
pub fn render_main_tf(target: &CloudTarget, spec: &Value) -> Result<String, String> {
    let Some(source) = provider_source(&target.cloud_category) else {
        return Err(format!(
            "cloud category {:?} has no curated tofu template yet",
            target.cloud_category
        ));
    };
    // The spec flows through terraform.tfvars.json; templates read it via
    // var.spec, so it is only sanity-checked here.
    if !spec.is_object() {
        return Err("spec must be a JSON object".into());
    }
    if target.cloud_category != DEMO_PROVIDER {
        validate_compute_spec(spec)?;
        if matches!(target.cloud_category.as_str(), "tencent" | "tencentcloud")
            && spec
                .get("vpc_id")
                .and_then(Value::as_str)
                .is_none_or(|v| v.trim().is_empty())
        {
            return Err("vpc_id is required for Tencent Cloud".into());
        }
        if target.region.trim().is_empty() {
            return Err("cloud region is required".into());
        }
        let (provider, body) = match target.cloud_category.as_str() {
            "aliyun" | "alicloud" => ("alicloud", include_str!("templates/aliyun.tf")),
            "tencent" | "tencentcloud" => ("tencentcloud", include_str!("templates/tencent.tf")),
            "huawei" | "huaweicloud" => ("huaweicloud", include_str!("templates/huawei.tf")),
            _ => return Err("unsupported compute provider".into()),
        };
        return Ok(format!(
            r#"terraform {{
  required_version = ">= 1.6"
  required_providers {{
    {provider} = {{ source = "{source}" }}
  }}
}}
variable "region" {{ type = string }}
variable "spec" {{ type = any }}
provider "{provider}" {{ region = var.region }}
{body}
"#
        ));
    }
    match target.cloud_category.as_str() {
        DEMO_PROVIDER => Ok(format!(
            r#"terraform {{
  required_version = ">= 1.6"
  required_providers {{
    null = {{
      source  = "{source}"
    }}
  }}
}}

variable "spec" {{
  type    = map(any)
  default = {{}}
}}

resource "null_resource" "provisioned" {{
  triggers = var.spec
}}

output "id" {{
  value = null_resource.provisioned.id
}}

output "spec" {{
  value = var.spec
}}
"#
        )),
        other => Err(format!(
            "cloud {other:?} provider source known but template not curated"
        )),
    }
}

/// Validate before starting a provider process. Values remain JSON data,
/// never interpolated into HCL source.
pub fn validate_compute_spec(spec: &Value) -> Result<(), String> {
    if let Some(size) = spec.get("system_disk_size_gb")
        && size.as_u64().is_none_or(|v| v == 0) {
            return Err("system_disk_size_gb must be a positive integer".into());
        }
    for key in [
        "ecs_name",
        "ecs_type",
        "image_id",
        "availability_zone",
        "subnet_id",
    ] {
        if spec
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(|v| v.trim().is_empty())
        {
            return Err(format!("{key} is required"));
        }
    }
    if spec
        .get("resource_count")
        .and_then(Value::as_u64)
        .is_none_or(|v| v == 0)
    {
        return Err("resource_count must be a positive integer".into());
    }
    if spec
        .get("security_groups")
        .and_then(Value::as_array)
        .is_none_or(|items| {
            items.is_empty()
                || items
                    .iter()
                    .any(|v| v.as_str().is_none_or(|s| s.trim().is_empty()))
        })
    {
        return Err("security_groups must be a nonempty array of IDs".into());
    }
    Ok(())
}

/// A read-only data source evaluated by `tofu plan`, never by apply.
pub fn render_connection_check(target: &CloudTarget) -> Result<String, String> {
    let (provider, query) = match target.cloud_category.as_str() {
        "aliyun" | "alicloud" => (
            "alicloud",
            "data \"alicloud_regions\" \"probe\" { current = true }",
        ),
        "tencent" | "tencentcloud" => (
            "tencentcloud",
            "data \"tencentcloud_regions\" \"probe\" { product = \"cvm\" }",
        ),
        "huawei" | "huaweicloud" => (
            "huaweicloud",
            "data \"huaweicloud_availability_zones\" \"probe\" {}",
        ),
        _ => return Err("该厂商尚不支持连接测试".into()),
    };
    let source = provider_source(&target.cloud_category).ok_or("unsupported provider")?;
    if target.region.trim().is_empty() {
        return Err("区域不能为空".into());
    }
    Ok(format!(
        r#"terraform {{
  required_providers {{
    {provider} = {{ source = "{source}" }}
  }}
}}
variable "region" {{ type = string }}
provider "{provider}" {{ region = var.region }}
{query}
"#
    ))
}

/// Render terraform.tfvars.json: the request spec under `spec` plus the
/// target region, matching the curated templates' variables.
pub fn render_tfvars(spec: &Value, region: &str) -> String {
    let spec = if spec.is_object() {
        spec.clone()
    } else {
        Value::Object(Map::new())
    };
    serde_json::to_string_pretty(&json!({ "spec": spec, "region": region }))
        .unwrap_or_else(|_| "{\"spec\": {}}".into())
}

/// Flatten `tofu output -json` into {name: value}.
pub fn parse_outputs(raw: &str) -> BTreeMap<String, Value> {
    let Value::Object(outputs) = serde_json::from_str::<Value>(raw).unwrap_or(Value::Null) else {
        return BTreeMap::new();
    };
    let mut flat = BTreeMap::new();
    for (name, entry) in outputs {
        if let Some(value) = entry.get("value") {
            flat.insert(name, value.clone());
        }
    }
    flat
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_provider_sources() {
        assert_eq!(provider_source("demo"), Some("hashicorp/null"));
        assert_eq!(provider_source(""), None);
        assert_eq!(provider_source("aliyun"), Some("aliyun/alicloud"));
        assert_eq!(
            provider_source("tencentcloud"),
            Some("tencentcloudstack/tencentcloud")
        );
        assert_eq!(provider_source("huawei"), Some("huaweicloud/huaweicloud"));
    }

    #[test]
    fn renders_demo_template_with_null_provider() {
        let target = CloudTarget {
            cloud_category: "demo".into(),
            provider_source: "hashicorp/null".into(),
            region: String::new(),
            access_key_id: String::new(),
            access_key_secret: String::new(),
        };
        let template = render_main_tf(&target, &json!({})).unwrap();
        assert!(template.contains("hashicorp/null"));
        assert!(template.contains("null_resource"));
        assert!(template.contains("output \"spec\""));
    }

    #[test]
    fn rejects_unsupported_clouds() {
        let target = CloudTarget {
            cloud_category: "unsupported".into(),
            provider_source: String::new(),
            region: String::new(),
            access_key_id: String::new(),
            access_key_secret: String::new(),
        };
        assert!(render_main_tf(&target, &json!({})).is_err());
    }

    #[test]
    fn renders_aliyun_credentials_env() {
        let target = CloudTarget {
            cloud_category: "aliyun".into(),
            provider_source: String::new(),
            region: "cn-hangzhou".into(),
            access_key_id: "ak".into(),
            access_key_secret: "sk".into(),
        };
        let env = credential_env(&target);
        assert!(env.contains(&("ALICLOUD_REGION".to_string(), "cn-hangzhou".to_string())));
        assert!(env.contains(&("ALICLOUD_ACCESS_KEY".to_string(), "ak".to_string())));
    }

    #[test]
    fn renders_tfvars_wrapping_spec() {
        let tfvars = render_tfvars(
            &json!({"ecs_name": "web-01", "cpu_cores": 8}),
            "cn-hangzhou",
        );
        let parsed: Value = serde_json::from_str(&tfvars).unwrap();
        assert_eq!(parsed["spec"]["ecs_name"], json!("web-01"));
        assert_eq!(parsed["spec"]["cpu_cores"], json!(8));
        assert_eq!(parsed["region"], json!("cn-hangzhou"));
    }

    #[test]
    fn flattens_output_json() {
        let raw = r#"{"id": {"sensitive": false, "value": "abc"}, "spec": {"value": {"a": 1}}}"#;
        let flat = parse_outputs(raw);
        assert_eq!(flat.get("id"), Some(&json!("abc")));
        assert_eq!(flat.get("spec"), Some(&json!({"a": 1})));
        assert!(parse_outputs("not-json").is_empty());
    }
}
