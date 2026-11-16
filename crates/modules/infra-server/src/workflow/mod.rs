//! 产品内流程引擎：定义（模板）→ 实例 → 步骤推进。
//!
//! v1 模型：顺序步骤 + 步骤级条件跳过（condition 对 payload/context 求值，
//! false → skipped）。节点类型：approval / manual / auto / notification /
//! ticket_wait。推进由 API 动作（审批/人工完成）与后台 runner（超时/重试/
//! 工单等待轮询）共同驱动；并发以实例级 advisory lock 串行化。
//! 与既有资源工单并存：`create_ticket` 自动节点把工单接入流程。

pub(crate) mod actions;
pub(crate) mod definition;
pub(crate) mod engine;
pub(crate) mod instance;
pub(crate) mod notify;
pub(crate) mod runner;

use aide::axum::ApiRouter;

use crate::InfraState;

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .merge(definition::routes())
        .merge(instance::routes())
}
