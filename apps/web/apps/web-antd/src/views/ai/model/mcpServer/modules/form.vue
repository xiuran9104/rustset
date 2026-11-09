<script lang="ts" setup>
import type { AiMcpServerApi } from '#/api/ai/model/mcpServer';

import { computed, ref } from 'vue';

import { useVbenModal } from '@vben/common-ui';

import { message } from 'ant-design-vue';

import { useVbenForm } from '#/adapter/form';
import {
  createMcpServer,
  getMcpServer,
  testMcpConnection,
  updateMcpServer,
} from '#/api/ai/model/mcpServer';
import { $t } from '#/locales';

import { useFormSchema } from '../data';

const emit = defineEmits(['success']);
const formData = ref<AiMcpServerApi.McpServer>();
const testing = ref(false);
const getTitle = computed(() => {
  return formData.value?.id
    ? $t('ui.actionTitle.edit', [' MCP 服务器'])
    : $t('ui.actionTitle.create', [' MCP 服务器']);
});

const [Form, formApi] = useVbenForm({
  commonConfig: {
    componentProps: {
      class: 'w-full',
    },
    labelWidth: 100,
  },
  wrapperClass: 'grid-cols-2',
  layout: 'horizontal',
  schema: useFormSchema(),
  showDefaultActions: false,
});

async function handleTest() {
  const values = await formApi.getValues();
  testing.value = true;
  try {
    const result = await testMcpConnection({
      url: String(values.url ?? ''),
      authHeader: values.authHeader ? String(values.authHeader) : undefined,
      authToken: values.authToken ? String(values.authToken) : undefined,
    });
    message.success(
      `连接成功：${result.serverInfo?.name ?? '未知'}（${result.toolCount} 个工具）`,
      4,
    );
  } catch (error: any) {
    message.error(`连接失败：${error?.message ?? error}`, 4);
  } finally {
    testing.value = false;
  }
}

const [Modal, modalApi] = useVbenModal({
  async onConfirm() {
    const { valid } = await formApi.validate();
    if (!valid) {
      return;
    }
    modalApi.lock();
    const data = (await formApi.getValues()) as AiMcpServerApi.McpServer;
    try {
      if (formData.value?.id) {
        // 编辑时留空令牌 = 保持不变（后端 COALESCE）
        if (!data.authToken) {
          delete data.authToken;
        }
        await updateMcpServer({ ...data, id: formData.value.id });
      } else {
        await createMcpServer(data);
      }
      await modalApi.close();
      emit('success');
      message.success($t('ui.actionMessage.operationSuccess'));
    } finally {
      modalApi.unlock();
    }
  },
  async onOpenChange(isOpen: boolean) {
    if (!isOpen) {
      formData.value = undefined;
      return;
    }
    const data = modalApi.getData<AiMcpServerApi.McpServer>();
    if (!data || !data.id) {
      return;
    }
    modalApi.lock();
    try {
      formData.value = await getMcpServer(data.id);
      await formApi.setValues({ ...formData.value, authToken: '' });
    } finally {
      modalApi.unlock();
    }
  },
});
</script>

<template>
  <Modal :title="getTitle" class="w-3/5">
    <Form class="mx-4" />
    <template #prepend-footer>
      <a-button :loading="testing" @click="handleTest">测试连接</a-button>
    </template>
  </Modal>
</template>
