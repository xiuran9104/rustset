<script lang="ts" setup>
import type { CmdbAttribute, CmdbModel } from '#/api/cmdb';

import { computed, onMounted, ref } from 'vue';

import { Page } from '@vben/common-ui';

import { message } from 'ant-design-vue';

import {
  ATTR_TYPES,
  createAttribute,
  createAttributeTrigger,
  createModel,
  deleteAttribute,
  deleteModel,
  getAttributesByModel,
  getAttributeTriggers,
  getModelList,
  updateAttribute,
  updateModel,
} from '#/api/cmdb';

const models = ref<CmdbModel[]>([]);
const loading = ref(false);
const drawerVisible = ref(false);
const editing = ref<CmdbModel | null>(null);
const modelForm = ref<CmdbModel>(defaultModel());

// 属性编辑器状态
const attributes = ref<CmdbAttribute[]>([]);
const attrDrawerVisible = ref(false);
const currentModel = ref<CmdbModel | null>(null);
const newAttr = ref<CmdbAttribute>(defaultAttr());
const choicesText = ref('');
const computedAttribute = ref(false);
const triggers = ref<any[]>([]);
const triggerForm = ref({ name: '', conditionCode: '', conditionValue: '', actionCode: '', actionValue: '', enabled: true });

function defaultModel(): CmdbModel {
  return { name: '', code: '', description: '', icon: '', uniqueKey: '', sort: 0 };
}
function defaultAttr(): CmdbAttribute {
  return {
    name: '',
    code: '',
    attrType: 'text',
    required: false,
    defaultValue: undefined,
    showInList: true,
    sort: 0,
  };
}

async function fetchModels() {
  loading.value = true;
  try {
    models.value = (await getModelList()) as any;
  } catch {
    message.error('加载模型失败');
  } finally {
    loading.value = false;
  }
}
onMounted(fetchModels);

function openCreate() {
  editing.value = null;
  modelForm.value = defaultModel();
  drawerVisible.value = true;
}
function openEdit(row: CmdbModel) {
  editing.value = row;
  modelForm.value = { ...row };
  drawerVisible.value = true;
}
async function submitModel() {
  try {
    if (editing.value) {
      await updateModel({ ...modelForm.value, id: editing.value.id! });
      message.success('模型已更新');
    } else {
      await createModel(modelForm.value);
      message.success('模型已创建，接下来请在属性管理中定义字段');
    }
    drawerVisible.value = false;
    fetchModels();
  } catch (error: any) {
    message.error(error?.message || '保存失败');
  }
}
async function removeModel(row: CmdbModel) {
  try {
    await deleteModel(row.id!);
    message.success('已删除');
    fetchModels();
  } catch (error: any) {
    message.error(error?.message || '删除失败');
  }
}

// ---------- 属性管理 ----------
function parseChoices(text: string): null | { label: string; value: string }[] {
  const items = text
    .split(/[\n,，]/)
    .map((item) => item.trim())
    .filter(Boolean)
    .map((item) => {
      const separator = item.search(/[:：]/);
      const label = separator >= 0 ? item.slice(0, separator).trim() : item;
      const value = separator >= 0 ? item.slice(separator + 1).trim() : '';
      return { label, value: value || label };
    });
  return items.length ? items : null;
}
function choicesToText(choices: CmdbAttribute['choices']): string {
  if (!choices?.length) return '';
  return choices.map((item) => `${item.label}:${item.value}`).join('\n');
}

function defaultValueToText(value: unknown): string {
  if (value === undefined || value === null) return '';
  return typeof value === 'string' ? value : JSON.stringify(value);
}

function normalizeDefaultValue(attr: CmdbAttribute): unknown {
  const value = attr.defaultValue;
  if (value === undefined || value === null || value === '') return null;
  if (typeof value !== 'string') return value;
  const text = value.trim();
  if (!text) return null;
  if (attr.attrType === 'number') {
    if (!/^-?\d+$/.test(text)) throw new Error('整数默认值格式不正确');
    return Number(text);
  }
  if (attr.attrType === 'float') {
    const parsed = Number(text);
    if (!Number.isFinite(parsed)) throw new Error('浮点数默认值格式不正确');
    return parsed;
  }
  if (attr.attrType === 'bool') {
    if (text === 'true') return true;
    if (text === 'false') return false;
    throw new Error('布尔默认值只能填写 true 或 false');
  }
  if (attr.attrType === 'json' || attr.attrType === 'multi_select') {
    try {
      return JSON.parse(text);
    } catch {
      throw new Error('JSON 默认值格式不正确');
    }
  }
  return text;
}

async function openAttributes(row: CmdbModel) {
  currentModel.value = row;
  attrDrawerVisible.value = true;
  attributes.value = ((await getAttributesByModel(row.id!)) as CmdbAttribute[]).map((attr) => ({
    ...attr,
    isComputed: Boolean(attr.expression),
  }));
  triggers.value = (await getAttributeTriggers(row.id!)) as any;
}
async function addTrigger() {
  if (!currentModel.value?.id) return;
  try {
    await createAttributeTrigger({ modelId: currentModel.value.id, ...triggerForm.value, conditionValue: JSON.parse(triggerForm.value.conditionValue), actionValue: JSON.parse(triggerForm.value.actionValue) });
    triggers.value = (await getAttributeTriggers(currentModel.value.id)) as any;
    triggerForm.value = { name: '', conditionCode: '', conditionValue: '', actionCode: '', actionValue: '', enabled: true };
    message.success('触发器已添加');
  } catch (error: any) { message.error(error?.message || '触发器保存失败'); }
}
async function reloadAttributes() {
  if (currentModel.value?.id) {
    attributes.value = ((await getAttributesByModel(currentModel.value.id)) as CmdbAttribute[]).map((attr) => ({
      ...attr,
      isComputed: Boolean(attr.expression),
    }));
  }
}
async function addAttribute() {
  if (!currentModel.value?.id) return;
  const attr = { ...newAttr.value, modelId: currentModel.value.id };
  if (computedAttribute.value) {
    if (!attr.expression?.trim()) {
      message.warning('请输入计算表达式');
      return;
    }
    if (!['float', 'number'].includes(attr.attrType)) {
      message.warning('计算属性类型只支持整数或浮点数');
      return;
    }
    attr.required = false;
    attr.defaultValue = undefined;
  } else {
    attr.expression = undefined;
  }
  if (attr.attrType === 'select' || attr.attrType === 'multi_select') {
    if (computedAttribute.value) {
      message.warning('计算属性只支持数值类型');
      return;
    }
    attr.choices = parseChoices(choicesText.value);
    if (!attr.choices) {
      message.warning('单选/多选属性需要至少一个选项（格式：标签:值，逗号或换行分隔）');
      return;
    }
  }
  try {
    await createAttribute({ ...attr, defaultValue: normalizeDefaultValue(attr) });
    message.success('属性已添加');
    newAttr.value = defaultAttr();
    computedAttribute.value = false;
    choicesText.value = '';
    reloadAttributes();
    fetchModels();
  } catch (error: any) {
    message.error(error?.message || '添加失败');
  }
}
async function saveAttribute(row: CmdbAttribute) {
  try {
    await updateAttribute({
      ...row,
      expression: row.isComputed ? row.expression : undefined,
      defaultValue: normalizeDefaultValue(row),
    } as CmdbAttribute & { id: number });
    message.success(`属性 ${row.name} 已保存`);
    fetchModels();
  } catch (error: any) {
    message.error(error?.message || '保存失败');
  }
}
async function removeAttribute(row: CmdbAttribute) {
  try {
    await deleteAttribute(row.id!);
    message.success('属性已删除');
    reloadAttributes();
    fetchModels();
  } catch (error: any) {
    message.error(error?.message || '删除失败');
  }
}

const attrTypeOptions = ATTR_TYPES;
const needsChoices = computed(
  () => newAttr.value.attrType === 'select' || newAttr.value.attrType === 'multi_select',
);

const columns = [
  { title: '模型名称', dataIndex: 'name', key: 'name', width: 150 },
  { title: '编码', dataIndex: 'code', key: 'code', width: 130 },
  { title: '唯一键', dataIndex: 'uniqueKey', key: 'uniqueKey', width: 110 },
  { title: '属性数', dataIndex: 'attributeCount', key: 'attributeCount', width: 80 },
  { title: '实例数', dataIndex: 'instanceCount', key: 'instanceCount', width: 80 },
  { title: '说明', dataIndex: 'description', key: 'description', ellipsis: true },
  { title: '排序', dataIndex: 'sort', key: 'sort', width: 70 },
  { title: '操作', key: 'actions', width: 260, fixed: 'right' },
];
</script>

<template>
  <Page auto-content-height>
    <div style="padding: 16px">
      <a-page-header
        title="模型管理"
        sub-title="自定义 CMDB 模型与属性（参考 veops/cmdb）"
        style="margin-bottom: 16px; padding: 0"
      />
      <a-space :size="16" style="margin-bottom: 16px">
        <a-button v-access:code="['cmdb:model:create']" type="primary" @click="openCreate">
          新建模型
        </a-button>
        <a-button @click="fetchModels">刷新</a-button>
      </a-space>
      <a-table
        :columns="columns"
        :data-source="models"
        :loading="loading"
        row-key="id"
        size="middle"
        :pagination="{ pageSize: 20 }"
      >
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'name'">
            <Icon :icon="record.icon || 'lucide:box'" style="margin-right: 6px; color: var(--ant-color-primary)" />
            {{ record.name }}
          </template>
          <template v-if="column.key === 'uniqueKey'">
            <a-tag v-if="record.uniqueKey" color="blue">{{ record.uniqueKey }}</a-tag>
            <span v-else>-</span>
          </template>
          <template v-if="column.key === 'actions'">
            <a-space>
              <a-button
                v-access:code="['cmdb:model:query']"
                type="link"
                size="small"
                @click="openAttributes(record)"
              >
                属性管理
              </a-button>
              <a-button
                v-access:code="['cmdb:model:update']"
                type="link"
                size="small"
                @click="openEdit(record)"
              >
                编辑
              </a-button>
              <a-popconfirm
                title="确认删除该模型？（需先清空其实例）"
                ok-text="删除"
                ok-type="danger"
                cancel-text="取消"
                @confirm="removeModel(record)"
              >
                <a-button v-access:code="['cmdb:model:delete']" type="link" size="small" danger>
                  删除
                </a-button>
              </a-popconfirm>
            </a-space>
          </template>
        </template>
      </a-table>

      <!-- 模型编辑 -->
      <a-modal
        v-model:open="drawerVisible"
        :title="editing ? '编辑模型' : '新建模型'"
        @ok="submitModel"
        width="560px"
      >
        <a-form v-if="modelForm" layout="vertical">
          <a-row :gutter="16">
            <a-col :span="12">
              <a-form-item label="模型名称" required>
                <a-input v-model:value="modelForm.name" placeholder="例如：物理服务器" />
              </a-form-item>
            </a-col>
            <a-col :span="12">
              <a-form-item label="模型编码" required>
                <a-input
                  v-model:value="modelForm.code"
                  :disabled="!!editing"
                  placeholder="例如：server（小写字母/数字/下划线）"
                />
              </a-form-item>
            </a-col>
            <a-col :span="12">
              <a-form-item label="唯一键属性编码" extra="用于识别同一模型下的唯一实例，如 hostname / sn">
                <a-input v-model:value="modelForm.uniqueKey" placeholder="例如：hostname" />
              </a-form-item>
            </a-col>
            <a-col :span="6">
              <a-form-item label="图标">
                <a-input v-model:value="modelForm.icon" placeholder="lucide:server" />
              </a-form-item>
            </a-col>
            <a-col :span="6">
              <a-form-item label="排序">
                <a-input-number v-model:value="modelForm.sort" :min="0" style="width: 100%" />
              </a-form-item>
            </a-col>
            <a-col :span="24">
              <a-form-item label="说明">
                <a-textarea v-model:value="modelForm.description" :rows="2" />
              </a-form-item>
            </a-col>
          </a-row>
        </a-form>
      </a-modal>

      <!-- 属性管理抽屉 -->
      <a-drawer
        v-model:open="attrDrawerVisible"
        :title="`属性管理 · ${currentModel?.name ?? ''}`"
        width="980"
      >
        <a-alert
          type="info"
          show-icon
          style="margin-bottom: 16px"
          message="属性定义了该模型实例的字段结构：类型、必填、选项与默认值。实例只能填写已定义的属性。"
        />
        <a-card size="small" title="新增属性" style="margin-bottom: 16px">
          <a-row :gutter="12">
            <a-input v-model:value="newAttr.color" type="color" style="width: 48px; padding: 2px; margin-bottom: 8px" />
            <a-col :span="5"><a-input v-model:value="newAttr.name" placeholder="属性名称" /></a-col>
            <a-col :span="5"><a-input v-model:value="newAttr.code" placeholder="属性编码" /></a-col>
            <a-col :span="4">
              <a-select v-model:value="newAttr.attrType" :options="attrTypeOptions as any" />
            </a-col>
            <a-col :span="2" style="text-align: center"><a-checkbox v-model:checked="newAttr.required">必填</a-checkbox></a-col>
            <a-col :span="2" style="text-align: center"><a-checkbox v-model:checked="newAttr.showInList">列表</a-checkbox></a-col>
            <a-col :span="2"><a-input-number v-model:value="newAttr.sort" :min="0" style="width: 100%" placeholder="排序" /></a-col>
            <a-col :span="4">
              <a-space>
                <a-checkbox v-model:checked="computedAttribute">计算属性</a-checkbox>
                <a-button v-access:code="['cmdb:attribute:create']" type="primary" @click="addAttribute">添加</a-button>
              </a-space>
            </a-col>
          </a-row>
          <a-textarea
            v-if="computedAttribute"
            v-model:value="newAttr.expression"
            :rows="2"
            style="margin-top: 8px"
            placeholder="公式示例：${cpu_count} * ${core_count} + 1"
          />
          <a-textarea
            v-if="needsChoices && !computedAttribute"
            v-model:value="choicesText"
            :rows="2"
            style="margin-top: 8px"
            placeholder="选项（每行一个，格式 标签:值，例如 核心:core）"
          />
          <a-input
            v-if="!computedAttribute"
            v-model:value="newAttr.defaultValue"
            style="margin-top: 8px"
            placeholder="默认值（整数/浮点/布尔按原值填写，多选和 JSON 使用 JSON 格式，可留空）"
          />
        </a-card>
        <a-table
          :columns="[
            { title: '属性名称', key: 'name', width: 150 },
            { title: '编码', key: 'code', width: 130 },
            { title: '类型', key: 'attrType', width: 120 },
            { title: '必填', key: 'required', width: 70 },
            { title: '列表显示', key: 'showInList', width: 90 },
            { title: '选项', key: 'choices', ellipsis: true },
            { title: '默认值', key: 'defaultValue', width: 150 },
            { title: '计算', key: 'isComputed', width: 85 },
            { title: '计算表达式', key: 'expression', width: 200 },
            { title: '字体颜色', key: 'color', width: 130 },
            { title: '排序', key: 'sort', width: 70 },
            { title: '操作', key: 'actions', width: 130 },
          ]"
          :data-source="attributes"
          row-key="id"
          size="small"
          :pagination="false"
        >
          <template #bodyCell="{ column, record }">
            <template v-if="column.key === 'name'">
              <a-input v-model:value="record.name" size="small" />
            </template>
            <template v-if="column.key === 'code'">
              <a-tag>{{ record.code }}</a-tag>
            </template>
            <template v-if="column.key === 'attrType'">
              <a-select v-model:value="record.attrType" size="small" :options="attrTypeOptions as any" style="width: 100%" />
            </template>
            <template v-if="column.key === 'required'">
              <a-switch v-model:checked="record.required" size="small" />
            </template>
            <template v-if="column.key === 'showInList'">
              <a-switch v-model:checked="record.showInList" size="small" />
            </template>
            <template v-if="column.key === 'choices'">
              <a-input
                :value="choicesToText(record.choices)"
                size="small"
                placeholder="标签:值 每行一个（仅单选/多选）"
                @change="(e: any) => (record.choices = parseChoices(e.target.value))"
              />
            </template>
            <template v-if="column.key === 'sort'">
              <a-input-number v-model:value="record.sort" size="small" :min="0" style="width: 100%" />
            </template>
            <template v-if="column.key === 'defaultValue'">
              <a-input
                :value="defaultValueToText(record.defaultValue)"
                size="small"
                placeholder="无默认值"
                @change="(e: any) => (record.defaultValue = e.target.value || undefined)"
              />
            </template>
            <template v-if="column.key === 'expression'">
              <a-input
                v-model:value="record.expression"
                size="small"
                :disabled="!record.isComputed"
                placeholder="${a} * ${b}"
              />
            </template>
            <template v-if="column.key === 'color'">
              <a-input v-model:value="record.color" type="color" size="small" style="width: 72px; padding: 2px" />
            </template>
            <template v-if="column.key === 'isComputed'">
              <a-switch v-model:checked="record.isComputed" size="small" />
            </template>
            <template v-if="column.key === 'actions'">
              <a-space>
                <a-button v-access:code="['cmdb:attribute:update']" type="link" size="small" @click="saveAttribute(record)">保存</a-button>
                <a-popconfirm title="确认删除该属性及所有实例中的对应字段值？" @confirm="removeAttribute(record)">
                  <a-button v-access:code="['cmdb:attribute:delete']" type="link" size="small" danger>删除</a-button>
                </a-popconfirm>
              </a-space>
            </template>
          </template>
        </a-table>
        <a-card size="small" title="属性触发器" style="margin-top: 16px">
          <a-row :gutter="8">
            <a-col :span="5"><a-input v-model:value="triggerForm.name" placeholder="规则名称" /></a-col>
            <a-col :span="4"><a-input v-model:value="triggerForm.conditionCode" placeholder="条件字段" /></a-col>
            <a-col :span="4"><a-input v-model:value="triggerForm.conditionValue" placeholder="条件值 JSON" /></a-col>
            <a-col :span="4"><a-input v-model:value="triggerForm.actionCode" placeholder="目标字段" /></a-col>
            <a-col :span="4"><a-input v-model:value="triggerForm.actionValue" placeholder="写入值 JSON" /></a-col>
            <a-col :span="3"><a-button type="primary" @click="addTrigger">添加</a-button></a-col>
          </a-row>
          <a-list v-if="triggers.length" size="small" :data-source="triggers" style="margin-top: 8px">
            <template #renderItem="{ item }"><a-list-item>{{ item.name }}：{{ item.conditionCode }} = {{ item.conditionValue }} → {{ item.actionCode }} = {{ item.actionValue }}</a-list-item></template>
          </a-list>
        </a-card>
      </a-drawer>
    </div>
  </Page>
</template>
