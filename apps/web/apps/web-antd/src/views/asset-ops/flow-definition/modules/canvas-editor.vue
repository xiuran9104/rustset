<script lang="ts" setup>
import type { Connection, Edge, Node } from '@vue-flow/core';
import type { AssetOpsFlowApi } from '#/api/asset-ops/flow';

import { nextTick, ref, watch } from 'vue';


import { Background } from '@vue-flow/background';
import { Controls } from '@vue-flow/controls';
import { MarkerType, VueFlow, useVueFlow } from '@vue-flow/core';
import dagre from '@dagrejs/dagre';

import { STEP_TYPES } from '../data';

interface EditorStep extends AssetOpsFlowApi.FlowStep {
  condField: string;
  condOp: string;
  condValue: string;
  nextText: string;
  [key: string]: any;
}

const props = defineProps<{ steps: EditorStep[] }>();
const emit = defineEmits(['update:steps']);

const { fitView, onConnect, onEdgeDoubleClick, onNodeDoubleClick } =
  useVueFlow();

const nodes = ref<Node[]>([]);
const edges = ref<Edge[]>([]);
const selectedStep = ref<EditorStep | null>(null);
const selectedKey = ref('');

const TYPE_COLORS: Record<string, string> = {
  llm: '#6a5acd',
  knowledge: '#20b2aa',
  agent: '#ff1493',
  approval: '#fa8c16',
  manual: '#52c41a',
  auto: '#1677ff',
  notification: '#8c8c8c',
  ticket_wait: '#722ed1',
};

// AI 资源选项（模型/知识库/工具/角色/Agent预设/通知模板）
const modelOptions = ref<{ label: string; value: number }[]>([]);
const knowledgeOptions = ref<{ label: string; value: number }[]>([]);
const toolOptions = ref<{ label: string; value: number }[]>([]);
const roleOptions = ref<{ label: string; value: string }[]>([]);
const agentPresetOptions = ref<{ label: string; value: number }[]>([]);
const notifyTemplateOptions = ref<{ label: string; value: string }[]>([]);

async function loadAIOptions() {
  const { requestClient } = await import('#/api/request');
  // 拉取系统角色列表
  requestClient.get('/system/role/simple-list')
    .then((list: any[]) => {
      roleOptions.value = (list ?? []).map((r) => ({
        label: `${r.name}（${r.code}）`,
        value: r.code,
      }));
    })
    .catch(() => {});
  // 拉取 Agent 预设列表
  requestClient.get('/ai/agent-preset/simple-list')
    .then((list: any[]) => {
      agentPresetOptions.value = (list ?? []).map((a) => ({
        label: `${a.name}（${a.description?.slice(0, 20) ?? ''}）`,
        value: a.id,
      }));
    })
    .catch(() => {});
  // 拉取通知模板列表
  requestClient.get('/system/notify-template/page', { params: { pageNo: 1, pageSize: 100 } })
    .then((page: any) => {
      notifyTemplateOptions.value = (page?.list ?? []).map((t: any) => ({
        label: `${t.name}（${t.code}）`,
        value: t.code,
      }));
    })
    .catch(() => {});
  // 拉取 chat 模型列表
  requestClient.get('/ai/model/simple-list', { params: { type: 'chat' } })
    .then((list: any[]) => {
      modelOptions.value = (list ?? []).map((m) => ({
        label: `${m.name}（${m.model}）`,
        value: m.id,
      }));
    })
    .catch(() => {});
  // 拉取知识库列表
  requestClient.get('/ai/knowledge/base/simple-list')
    .then((list: any[]) => {
      knowledgeOptions.value = (list ?? []).map((kb) => ({
        label: kb.name,
        value: kb.id,
      }));
    })
    .catch(() => {});
  // 拉取工具列表
  requestClient.get('/ai/tool/simple-list')
    .then((list: any[]) => {
      toolOptions.value = (list ?? []).map((t) => ({
        label: t.name,
        value: t.id,
      }));
    })
    .catch(() => {});
}
loadAIOptions();

// steps → nodes + edges
function syncToCanvas() {
  const g = new dagre.graphlib.Graph();
  g.setGraph({ rankdir: 'LR', nodesep: 60, ranksep: 100 });
  g.setDefaultEdgeLabel(() => ({}));

  const nodeList: Node[] = props.steps.map((step, i) => ({
    id: step.key,
    position: { x: i * 200, y: 0 },
    data: { label: step.name || step.key, type: step.type },
    style: {
      background: '#fff',
      border: `2px solid ${TYPE_COLORS[step.type] || '#ccc'}`,
      borderRadius: '8px',
      padding: '8px 12px',
      fontSize: '12px',
      width: '140px',
    },
  }));
  nodeList.forEach((n) => g.setNode(n.id, { width: 140, height: 50 }));

  const edgeList: Edge[] = [];
  props.steps.forEach((step) => {
    try {
      const next = step.nextText ? JSON.parse(step.nextText) : [];
      if (Array.isArray(next)) {
        next.forEach((edge: any) => {
          if (edge.to && edge.to !== step.key) {
            edgeList.push({
              id: `${step.key}->${edge.to}`,
              source: step.key,
              target: edge.to,
              animated: true,
              markerEnd: MarkerType.ArrowClosed,
              label: edge.when ? '条件' : undefined,
              style: { stroke: '#999' },
            });
            g.setEdge(step.key, edge.to);
          }
        });
      }
    } catch {
      // JSON 无效时跳过
    }
  });

  // dagre 自动布局
  dagre.layout(g);
  nodeList.forEach((node) => {
    const pos = g.node(node.id);
    if (pos) {
      node.position = { x: pos.x - 70, y: pos.y - 25 };
    }
  });

  nodes.value = nodeList;
  edges.value = edgeList;

  nextTick(() => fitView({ padding: 0.2 }));
}

// nodes + edges → steps
function syncFromCanvas(): EditorStep[] {
  const stepMap = new Map(props.steps.map((s) => [s.key, s]));
  const result: EditorStep[] = [];
  for (const node of nodes.value) {
    const step = stepMap.get(node.id);
    if (!step) continue;
    const outgoing = (edges.value as any[]).filter(
      (e: { source: string }) => e.source === node.id,
    );
    step.nextText =
      outgoing.length === 0
        ? ''
        : JSON.stringify(outgoing.map((e) => ({ to: e.target })));
    result.push(step);
  }
  return result;
}

// 连线 → 添加出口
onConnect((connection: Connection) => {
  const source = props.steps.find((s) => s.key === connection.source);
  const target = props.steps.find((s) => s.key === connection.target);
  if (!source || !target || source.key === target.key) return;
  try {
    const next = source.nextText ? JSON.parse(source.nextText) : [];
    if (Array.isArray(next) && !next.some((n: any) => n.to === target.key)) {
      next.push({ to: target.key });
      source.nextText = JSON.stringify(next);
    }
  } catch {
    source.nextText = JSON.stringify([{ to: target.key }]);
  }
  edges.value.push({
    id: `${connection.source}->${connection.target}`,
    source: connection.source,
    target: connection.target,
    animated: true,
    markerEnd: MarkerType.ArrowClosed,
  });
  emitUpdate();
});

// 双击边 → 删除出口
onEdgeDoubleClick(({ edge }) => {
  const source = props.steps.find((s) => s.key === edge.source);
  if (source) {
    try {
      const next = source.nextText ? JSON.parse(source.nextText) : [];
      const filtered = next.filter((n: any) => n.to !== edge.target);
      source.nextText = filtered.length ? JSON.stringify(filtered) : '';
    } catch {
      // skip
    }
  }
  edges.value = (edges.value as any[]).filter((e: { id: string }) => e.id !== edge.id);
  emitUpdate();
});

// 双击节点 → 属性面板
onNodeDoubleClick(({ node }) => {
  selectedKey.value = node.id;
  selectedStep.value =
    props.steps.find((s) => s.key === node.id) ?? null;
});

function emitUpdate() {
  emit('update:steps', syncFromCanvas());
}

function addStep() {
  const index = props.steps.length;
  const key = `step_${index + 1}`;
  const newStep: EditorStep = {
    key,
    name: `新步骤 ${index + 1}`,
    type: 'approval',
    config: { assignee: { role: 'super_admin' } },
    condition: null,
    condField: '',
    condOp: 'eq',
    condValue: '',
    nextText: '',
  };
  props.steps.push(newStep);
  syncToCanvas();
}

function removeStep(key: string) {
  const idx = props.steps.findIndex((s) => s.key === key);
  if (idx >= 0) {
    props.steps.splice(idx, 1);
    // 清除所有指向该步骤的出口
    props.steps.forEach((s) => {
      try {
        const next = s.nextText ? JSON.parse(s.nextText) : [];
        s.nextText = JSON.stringify(next.filter((n: any) => n.to !== key));
        if (s.nextText === '[]') s.nextText = '';
      } catch {
        // skip
      }
    });
    if (selectedKey.value === key) {
      selectedStep.value = null;
      selectedKey.value = '';
    }
    syncToCanvas();
    emitUpdate();
  }
}

watch(
  () => props.steps.map((s) => `${s.key}:${s.type}:${s.nextText}`).join('|'),
  () => syncToCanvas(),
  { immediate: true },
);
</script>

<template>
  <div class="flex h-[520px] gap-3">
    <!-- 画布 -->
    <div class="flex-1 rounded border border-gray-200 dark:border-gray-700">
      <VueFlow
        :nodes="nodes"
        :edges="edges"
        :default-viewport="{ zoom: 1 }"
        :min-zoom="0.3"
        :max-zoom="2"
        fit-view-on-init
      >
        <Background pattern-color="#aaa" :gap="16" />
        <Controls position="bottom-left" />
      </VueFlow>
    </div>

    <!-- 属性面板（宽面板 + 滚动，防溢出） -->
    <div
      v-if="selectedStep"
      class="w-96 max-h-[520px] overflow-y-auto rounded border border-gray-200 p-3 dark:border-gray-700"
    >
      <div class="mb-2 flex items-center justify-between">
        <span class="text-sm font-medium">节点属性</span>
        <a-button danger size="small" @click="removeStep(selectedStep.key)">
          删除节点
        </a-button>
      </div>
      <div class="space-y-2">
        <div>
          <div class="mb-1 text-xs text-gray-500">key</div>
          <a-input v-model:value="selectedStep.key" size="small" disabled />
        </div>
        <div>
          <div class="mb-1 text-xs text-gray-500">名称</div>
          <a-input v-model:value="selectedStep.name" size="small" />
        </div>
        <div>
          <div class="mb-1 text-xs text-gray-500">类型</div>
          <a-select
            :value="selectedStep.type"
            class="w-full"
            size="small"
            @change="(v: any) => (selectedStep!.type = v)"
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
        <div v-if="['approval', 'manual', 'notification'].includes(selectedStep.type)">
          <div class="mb-1 text-xs text-gray-500">办理角色</div>
          <a-select
            :value="selectedStep.config.assignee?.role"
            class="w-full"
            size="small"
            placeholder="选择角色"
            show-search
            :filter-option="(input: string, option: any) => option.label?.toLowerCase().includes(input.toLowerCase())"
            @change="(v: any) => {
              if (!selectedStep!.config.assignee) selectedStep!.config.assignee = {};
              selectedStep!.config.assignee.role = v;
            }"
          >
            <a-select-option
              v-for="r in roleOptions"
              :key="r.value"
              :value="r.value"
              :label="r.label"
            >
              {{ r.label }}
            </a-select-option>
          </a-select>
        </div>
        <!-- 审批/人工节点：动态审批人（fromContext） -->
        <div v-if="['approval', 'manual'].includes(selectedStep.type)">
          <div class="mb-1 text-xs text-gray-500">动态办理人（可选）</div>
          <a-input
            v-model:value="selectedStep.config.fromContext"
            size="small"
            placeholder="context 中的用户名字段，如 risk_analysis.approver_username"
          />
          <div class="mt-1 text-xs text-gray-400">
            激活时从 context 按点路径取用户名，并入办理人（点路径不含花括号）
          </div>
        </div>
        <!-- 通知节点：模板选择 -->
        <div v-if="selectedStep.type === 'notification'">
          <div class="mb-1 text-xs text-gray-500">通知模板</div>
          <a-select
            :value="selectedStep.config.templateCode"
            class="w-full"
            size="small"
            placeholder="选择通知模板"
            show-search
            :filter-option="(input: string, option: any) => option.label?.toLowerCase().includes(input.toLowerCase())"
            @change="(v: any) => (selectedStep!.config.templateCode = v)"
          >
            <a-select-option
              v-for="t in notifyTemplateOptions"
              :key="t.value"
              :value="t.value"
              :label="t.label"
            >
              {{ t.label }}
            </a-select-option>
          </a-select>
        </div>
        <!-- 通知节点：模板自定义参数（插值后做 {key} 占位符替换） -->
        <div v-if="selectedStep.type === 'notification'">
          <div class="mb-1 text-xs text-gray-500">
            模板参数 JSON
            <span class="text-blue-400" v-pre>（值可用 {{payload.x}} {{context.x}}）</span>
          </div>
          <a-textarea
            :rows="3"
            :value="JSON.stringify(selectedStep.config.params ?? {})"
            size="small"
            placeholder='{"ticketName":"{{payload.ecsName}}"}'
            @change="(e: any) => {
              try { selectedStep!.config.params = JSON.parse(e.target.value || '{}'); } catch {}
            }"
          />
          <div class="mt-1 text-xs text-gray-400" v-pre>
            参数键对应模板里的 {key} 占位符，如 ticketName → {ticketName}
          </div>
        </div>
        <div v-if="selectedStep.type === 'auto'">
          <div class="mb-1 text-xs text-gray-500">自动动作</div>
          <a-select
            v-model:value="selectedStep.config.action"
            class="w-full"
            size="small"
          >
            <a-select-option value="http_request">HTTP 请求</a-select-option>
            <a-select-option value="start_flow">启动子流程</a-select-option>
            <a-select-option value="firewall_push">防火墙下发</a-select-option>
            <a-select-option value="trigger_scan">触发扫描</a-select-option>
            <a-select-option value="create_ticket">创建工单</a-select-option>
            <a-select-option value="set_context">写上下文</a-select-option>
            <a-select-option value="ticket_approve">工单审批</a-select-option>
            <a-select-option value="ticket_provision">工单供应</a-select-option>
            <a-select-option value="ticket_deliver">工单交付</a-select-option>
          </a-select>
        </div>
        <div v-if="selectedStep.type === 'auto'">
          <div class="mb-1 text-xs text-gray-500">
            参数 JSON
            <span class="text-blue-400" v-pre>（值可用 {{payload.x}} {{context.x}} {{steps.key.output.x}}，类型保留）</span>
          </div>
          <a-textarea
            :rows="3"
            :value="JSON.stringify(selectedStep.config.params ?? {})"
            size="small"
            placeholder='{"url":"https://api.example.com/host/{{payload.ecsName}}"}'
            @change="(e: any) => {
              try { selectedStep!.config.params = JSON.parse(e.target.value || '{}'); } catch {}
            }"
          />
        </div>
        <!-- LLM 节点 -->
        <template v-if="selectedStep.type === 'llm'">
          <div>
            <div class="mb-1 text-xs text-gray-500">模型</div>
            <a-select
              :value="selectedStep.config.modelId"
              class="w-full"
              size="small"
              placeholder="选择 AI 模型"
              @change="(v: any) => (selectedStep!.config.modelId = v)"
            >
              <a-select-option v-for="m in modelOptions" :key="m.value" :value="m.value">
                {{ m.label }}
              </a-select-option>
            </a-select>
          </div>
          <div>
            <div class="mb-1 text-xs text-gray-500">系统提示词</div>
            <a-textarea
              v-model:value="selectedStep.config.systemPrompt"
              :rows="3"
              size="small"
              placeholder="你是安全分析师..."
            />
          </div>
          <div>
            <div class="mb-1 text-xs text-gray-500">
              用户提示词
              <span class="text-blue-400" v-pre>（可用 {{payload.x}} {{context.x}}）</span>
            </div>
            <a-textarea
              v-model:value="selectedStep.config.userPrompt"
              :rows="3"
              size="small"
              placeholder="分析 {{payload.ecsName}} 的风险"
            />
          </div>
          <div class="flex gap-2">
            <div class="flex-1">
              <div class="mb-1 text-xs text-gray-500">温度</div>
              <a-input-number
                v-model:value="selectedStep.config.temperature"
                :min="0" :max="2" :step="0.1"
                size="small" class="!w-full"
              />
            </div>
            <div class="flex-1">
              <div class="mb-1 text-xs text-gray-500">最大 Token</div>
              <a-input-number
                v-model:value="selectedStep.config.maxTokens"
                :min="100" :max="32000"
                size="small" class="!w-full"
              />
            </div>
          </div>
          <div class="flex gap-2">
            <div class="flex-1">
              <div class="mb-1 text-xs text-gray-500">输出格式</div>
              <a-select
                :value="selectedStep.config.outputFormat ?? 'text'"
                class="w-full" size="small"
                @change="(v: any) => (selectedStep!.config.outputFormat = v)"
              >
                <a-select-option value="text">文本</a-select-option>
                <a-select-option value="json">JSON 提取</a-select-option>
              </a-select>
            </div>
            <div class="flex-1">
              <div class="mb-1 text-xs text-gray-500">输出变量名</div>
              <a-input
                v-model:value="selectedStep.config.outputKey"
                size="small" placeholder="llm_output"
              />
            </div>
          </div>
        </template>
        <!-- 知识检索节点 -->
        <template v-if="selectedStep.type === 'knowledge'">
          <div>
            <div class="mb-1 text-xs text-gray-500">知识库</div>
            <a-select
              :value="selectedStep.config.knowledgeIds ?? []"
              mode="multiple"
              class="w-full"
              size="small"
              placeholder="选择知识库"
              @change="(v: any) => (selectedStep!.config.knowledgeIds = v)"
            >
              <a-select-option v-for="kb in knowledgeOptions" :key="kb.value" :value="kb.value">
                {{ kb.label }}
              </a-select-option>
            </a-select>
          </div>
          <div>
            <div class="mb-1 text-xs text-gray-500">
              检索内容
              <span class="text-blue-400" v-pre>（支持 {{payload.x}}）</span>
            </div>
            <a-textarea
              v-model:value="selectedStep.config.query"
              :rows="2"
              size="small"
              placeholder="{{payload.resourceType}} {{payload.ecsName}}"
            />
          </div>
          <div class="flex gap-2">
            <div class="flex-1">
              <div class="mb-1 text-xs text-gray-500">Top K</div>
              <a-input-number
                v-model:value="selectedStep.config.topK"
                :min="1" :max="20"
                size="small" class="!w-full"
              />
            </div>
            <div class="flex-1">
              <div class="mb-1 text-xs text-gray-500">输出变量名</div>
              <a-input
                v-model:value="selectedStep.config.outputKey"
                size="small" placeholder="kb_context"
              />
            </div>
          </div>
        </template>
        <!-- Agent 节点：选择预配置的 Agent -->
        <template v-if="selectedStep.type === 'agent'">
          <div>
            <div class="mb-1 text-xs text-gray-500">Agent 预设</div>
            <a-select
              :value="selectedStep.config.agentPresetId"
              class="w-full"
              size="small"
              placeholder="选择已配置的 AI Agent"
              show-search
              :filter-option="(input: string, option: any) => option.label?.toLowerCase().includes(input.toLowerCase())"
              @change="(v: any) => (selectedStep!.config.agentPresetId = v)"
            >
              <a-select-option
                v-for="a in agentPresetOptions"
                :key="a.value"
                :value="a.value"
                :label="a.label"
              >
                {{ a.label }}
              </a-select-option>
            </a-select>
            <div class="mt-1 text-xs text-gray-400">
              在「AI 大模型 → AI Agent 管理」中配置模型/提示词/工具
            </div>
          </div>
          <div>
            <div class="mb-1 text-xs text-gray-500">
              用户提示词
              <span class="text-blue-400" v-pre>（支持 {{payload.x}}）</span>
            </div>
            <a-textarea
              v-model:value="selectedStep.config.userPrompt"
              :rows="2"
              size="small"
              placeholder="工单：{{payload.ecsName}}"
            />
          </div>
          <div>
            <div class="mb-1 text-xs text-gray-500">输出变量名</div>
            <a-input
              v-model:value="selectedStep.config.outputKey"
              size="small" placeholder="agent_result"
            />
          </div>
        </template>
        <div>
          <div class="mb-1 text-xs text-gray-500">出口（JSON）</div>
          <a-textarea
            v-model:value="selectedStep.nextText"
            :rows="2"
            size="small"
            placeholder='[{"to":"s2"}] 或留空=线性'
          />
        </div>
      </div>
    </div>

    <!-- 未选中提示 -->
    <div
      v-else
      class="flex w-72 items-center justify-center rounded border border-dashed border-gray-300 text-sm text-gray-400 dark:border-gray-600"
    >
      双击节点编辑属性<br />拖拽连线创建出口<br />双击边删除出口
    </div>
  </div>

  <!-- 添加步骤按钮 -->
  <div class="mt-2 text-right">
    <a-button size="small" @click="addStep">+ 添加节点</a-button>
  </div>
</template>

<style>
/* Vue Flow 基础样式 */
@import '@vue-flow/core/dist/style.css';
@import '@vue-flow/core/dist/theme-default.css';
@import '@vue-flow/controls/dist/style.css';
</style>
