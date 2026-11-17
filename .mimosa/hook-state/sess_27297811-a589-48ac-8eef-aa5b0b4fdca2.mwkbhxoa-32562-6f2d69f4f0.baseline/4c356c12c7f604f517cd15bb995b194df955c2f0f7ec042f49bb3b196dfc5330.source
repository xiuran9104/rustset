use rustset_framework_tofu::{
    CloudTarget, TofuExecutor, credential_env, provider_source, render_connection_check,
    render_main_tf,
};
use serde_json::{Value, json};

fn target(provider: &str) -> CloudTarget {
    CloudTarget {
        cloud_category: provider.into(),
        provider_source: provider_source(provider).unwrap_or_default().into(),
        region: "test-region".into(),
        access_key_id: "test-access-key".into(),
        access_key_secret: "test-secret-key".into(),
    }
}

fn spec() -> Value {
    json!({"ecs_name":"web", "ecs_type":"flavor-id", "image_id":"image-id",
        "availability_zone":"zone-id", "subnet_id":"subnet-id", "vpc_id":"vpc-id",
        "security_groups":["sg-id"], "resource_count":2})
}

#[test]
fn rejects_missing_network_image_and_invalid_counts() {
    for provider in ["aliyun", "tencent", "huawei"] {
        for key in [
            "image_id",
            "availability_zone",
            "subnet_id",
            "security_groups",
            "resource_count",
        ] {
            let mut invalid = spec();
            invalid.as_object_mut().unwrap().remove(key);
            assert!(
                render_main_tf(&target(provider), &invalid).is_err(),
                "{provider}: {key}"
            );
        }
        for count in [json!(0), json!(-1), json!(1.5), json!("2")] {
            let mut invalid = spec();
            invalid["resource_count"] = count;
            assert!(render_main_tf(&target(provider), &invalid).is_err());
        }
    }
    let mut invalid = spec();
    invalid.as_object_mut().unwrap().remove("vpc_id");
    assert!(render_main_tf(&target("tencent"), &invalid).is_err());
}

#[test]
fn secrets_and_user_values_never_enter_hcl_source() {
    for provider in ["aliyun", "tencent", "huawei"] {
        let mut input = spec();
        input["ecs_name"] = json!("\"} resource \"unexpected\" { #");
        let cloud = target(provider);
        let template = render_main_tf(&cloud, &input).unwrap();
        assert!(!template.contains("unexpected"));
        assert!(!template.contains(&cloud.access_key_secret));
        assert!(
            credential_env(&cloud)
                .iter()
                .any(|(_, value)| value == &cloud.access_key_secret)
        );
    }
    assert!(render_main_tf(&target(""), &spec()).is_err());
}

/// Downloads actual providers and validates their resource schemas; no cloud
/// credentials, plan or apply. Run explicitly when network access is available.
#[tokio::test]
#[ignore = "requires tofu and provider registry downloads"]
async fn templates_validate_against_real_provider_schemas() {
    let executor = TofuExecutor::from_env();
    assert!(executor.available().await, "tofu is required");
    for provider in ["aliyun", "tencent", "huawei"] {
        let workspace = executor
            .ensure_workspace(&format!("schema-test-{}-{provider}", std::process::id()))
            .unwrap();
        let template = render_main_tf(&target(provider), &spec()).unwrap();
        TofuExecutor::write_file(&workspace, "main.tf", &template).unwrap();
        let init = executor.init(&workspace).await;
        assert!(init.success, "{provider}: {}", init.combined_log());
        let validate = executor
            .run(&["validate", "-no-color"], &[], &workspace)
            .await;
        assert!(validate.success, "{provider}: {}", validate.combined_log());
        TofuExecutor::write_file(
            &workspace,
            "main.tf",
            &render_connection_check(&target(provider)).unwrap(),
        )
        .unwrap();
        let validate_probe = executor
            .run(&["validate", "-no-color"], &[], &workspace)
            .await;
        assert!(
            validate_probe.success,
            "{provider} probe: {}",
            validate_probe.combined_log()
        );
    }
}
