<script lang="ts" setup>
import type { AssetOpsFlowApi } from '#/api/asset-ops/flow';

import { computed, reactive, ref } from 'vue';

import { message } from 'ant-design-vue';

import {
  AUTO_ACTIONS,
  CONDITION_OPS,
  STEP_TYPES,
} from '../data';
import {
  createFlowDefinition,
  getFlowDefinition,
  updateFlowDefinition,
} from '#/api/asset-ops/flow';

const emit = defineEmits(['success']);

interface EditorStep extends AssetOpsFlowApi.FlowStep {
  /** 条件编辑态（空串=无条件） */
  condField: string;
  condOp: string;
  condValue: string;
}

const visible = ref(false);
const editingId = ref<null | number>(null);
const saving = ref(false);
const form = reactive({
  name: '',
  description: '',
  status: 0,
});
const steps = ref<EditorStep[]>([]);

const title = computed(() =>
  editingId.value ? '编辑流程定义' : '新建流程定义',
);

function emptyStep(index: number): EditorStep {
  return {
    key: `step_${index + 1}`,
    name: '',
    type: 'approval',
    config: { assignee: { role: 'super_admin' } },
    condition: null,
    condField: '',
    condOp: 'eq',
    condValue: '',
  };
}

function open(row?: AssetOpsFlowApi.FlowDefinition) {
  visible.value = true;
  steps.value = [];
  if (row?.id) {
    editingId.value = row.id;
    getFlowDefinition(row.id).then((def) => {
      form.name = def.name;
      form.description = def.description;
      form.status = def.status;
      steps.value = (def.definition?.steps ?? []).map((s) => ({
        ...s,
        config: { assignee: { role: 'super_admin' }, ...s.config },
        condField: s.condition?.field ?? '',
        condOp: s.condition?.op ?? 'eq',
        condValue: s.condition ? JSON.stringify(s.condition.value) : '',
      }));
    });
  } else {
    editingId.value = null;
    form.name = '';
    form.description = '';
    form.status = 0;
    steps.value = [emptyStep(0)];
  }
}

function onTypeChange(step: EditorStep) {
  // 类型切换给默认 config
  if (step.type === 'approval' || step.type === 'manual') {
    step.config = { assignee: { role: 'super_admin' } };
  } else if (step.type === 'auto') {
    step.config = { action: 'set_context', params: {} };
  } else if (step.type === 'notification') {
    step.config = { assignee: { role: 'super_admin' } };
  } else if (step.type === 'ticket_wait') {
    step.config = {};
  }
}

function buildDefinition() {
  return {
    steps: steps.value.map((step) => {
      let condition = null;
      if (step.condField.trim()) {
        let value: any = step.condValue;
        try {
          value = JSON.parse(step.condValue);
        } catch {
          // 保持字符串
        }
        condition = { field: step.condField.trim(), op: step.condOp, value };
      }
      const config = { ...step.config };
      return { ...step, config, condition };
    }),
  };
}

async function handleSave() {
  if (!form.name.trim()) {
    message.error('请填写流程名称');
    return;
  }
  if (steps.value.length === 0) {
    message.error('至少一个步骤');
    return;
  }
  saving.value = true;
  try {
    const payload = {
      id: editingId.value ?? undefined,
      name: form.name,
      description: form.description,
      status: form.status,
      definition: buildDefinition(),
    } as AssetOpsFlowApi.FlowDefinition;
    if (editingId.value) {
      await updateFlowDefinition(payload);
    } else {
      await createFlowDefinition(payload);
    }
    message.success('已保存');
    visible.value = false;
    emit('success');
  } catch (error: any) {
    message.error(error?.message ?? '保存失败');
  } finally {
    saving.value = false;
  }
}

defineExpose({ open });
</script>

<template>
  <a-modal
    v-model:open="visible"
    :title="title"
    width="980px"
    :footer="null"
    :mask-closable="false"
  >
    <div class="mb-4 grid grid-cols-3 gap-3">
      <div>
        <div class="mb-1 text-sm">流程名称 *</div>
        <a-input v-model:value="form.name" placeholder="例如：网络开通标准流程" />
      </div>
      <div>
        <div class="mb-1 text-sm">说明</div>
        <a-input v-model:value="form.description" placeholder="可选" />
      </div>
      <div>
        <div class="mb-1 text-sm">状态</div>
        <a-select v-model:value="form.status" class="w-full">
          <a-select-option :value="0">启用</a-select-option>
          <a-select-option :value="1">停用</a-select-option>
        </a-select>
      </div>
    </div>

    <div class="mb-2 flex items-center justify-between">
      <div class="text-sm font-medium">步骤（按顺序执行；条件为假时跳过）</div>
      <a-button size="small" @click="steps.push(emptyStep(steps.length))">
        + 添加步骤
      </a-button>
    </div>

    <div
      v-for="(step, i) in steps"
      :key="i"
      class="mb-3 rounded border border-gray-200 p-3 dark:border-gray-700"
    >
      <div class="mb-2 grid grid-cols-12 gap-2">
        <div class="col-span-3">
          <a-input v-model:value="step.key" placeholder="key（小写/数字/_）" />
        </div>
        <div class="col-span-4">
          <a-input v-model:value="step.name" placeholder="步骤名称" />
        </div>
        <div class="col-span-3">
          <a-select
            :value="step.type"
            class="w-full"
            @change="(v: any) => { step.type = v; onTypeChange(step); }"
          >
            <a-select-option
              v-for="t in STEP_TYPES"
              :key="t.value"
              :value="t.value"
            >
              {{ t.label }}
            </a-select-option>
          </a-select>
        </div>
        <div class="col-span-2 text-right">
          <a-button danger size="small" @click="steps.splice(i, 1)">
            删除
          </a-button>
        </div>
      </div>

      <div class="grid grid-cols-12 gap-2">
        <!-- 审批/人工/通知：assignee + 超时 -->
        <template
          v-if="['approval', 'manual', 'notification'].includes(step.type)"
        >
          <div class="col-span-4">
            <a-input
              v-model:value="step.config.assignee!.role"
              placeholder="办理角色（如 super_admin）"
            />
          </div>
          <template v-if="['approval', 'manual'].includes(step.type)">
            <div class="col-span-2">
              <a-input-number
                v-model:value="step.config.timeoutHours"
                placeholder="超时(小时)"
                class="w-full"
                :min="0"
              />
            </div>
            <div class="col-span-3">
              <a-select
                v-model:value="step.config.onTimeout"
                class="w-full"
                placeholder="超时策略"
                allow-clear
              >
                <a-select-option value="escalate_notify">
                  超时升级通知
                </a-select-option>
                <a-select-option value="auto_reject">
                  超时自动拒绝
                </a-select-option>
              </a-select>
            </div>
          </template>
        </template>

        <!-- 自动动作 -->
        <template v-if="step.type === 'auto'">
          <div class="col-span-4">
            <a-select v-model:value="step.config.action" class="w-full">
              <a-select-option
                v-for="a in AUTO_ACTIONS"
                :key="a.value"
                :value="a.value"
              >
                {{ a.label }}
              </a-select-option>
            </a-select>
          </div>
          <div class="col-span-8">
            <a-textarea
              :rows="1"
              placeholder='动作参数 JSON，如 {"target":"10.0.0.1","ports":[22,3389]}'
              :value="JSON.stringify(step.config.params ?? {})"
              @change="
                (e: any) => {
                  try {
                    step.config.params = JSON.parse(e.target.value || '{}');
                  } catch {
                    /* 输入中，暂不解析 */
                  }
                }
              "
            />
          </div>
        </template>

        <!-- 条件（全部类型可配） -->
        <div class="col-span-5">
          <div class="flex gap-1">
            <a-input v-model:value="step.condField" placeholder="条件字段（空=无条件）" />
            <a-select v-model:value="step.condOp" style="width: 110px">
              <a-select-option
                v-for="op in CONDITION_OPS"
                :key="op.value"
                :value="op.value"
              >
                {{ op.label }}
              </a-select-option>
            </a-select>
            <a-input v-model:value="step.condValue" placeholder='值，如 "high" 或 8080' />
          </div>
        </div>
      </div>
    </div>

    <div class="text-right">
      <a-button class="mr-2" @click="visible = false">取消</a-button>
      <a-button type="primary" :loading="saving" @click="handleSave">
        保存
      </a-button>
    </div>
  </a-modal>
</template>
