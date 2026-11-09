<script lang="ts" setup>
import { useVbenModal } from '@vben/common-ui';

import { useVbenForm } from '#/adapter/form';
import { createMcpKey } from '#/api/system/mcp-key';

import { useFormSchema } from '../data';

const emit = defineEmits(['success']);

const [Form, formApi] = useVbenForm({
  commonConfig: {
    componentProps: {
      class: 'w-full',
    },
    labelWidth: 120,
  },
  wrapperClass: 'grid-cols-1',
  layout: 'horizontal',
  schema: useFormSchema(),
  showDefaultActions: false,
});

const [Modal, modalApi] = useVbenModal({
  async onConfirm() {
    const { valid } = await formApi.validate();
    if (!valid) {
      return;
    }
    modalApi.lock();
    try {
      const values = await formApi.getValues();
      const created = await createMcpKey({
        name: String(values.name ?? ''),
        userId: Number(values.userId),
        expiresTime: values.expiresTime || undefined,
      });
      await modalApi.close();
      // 把一次性明文交给父组件展示
      emit('success', { apiKey: created.apiKey, name: String(values.name) });
    } finally {
      modalApi.unlock();
    }
  },
  async onOpenChange(isOpen: boolean) {
    if (!isOpen) {
      return;
    }
    await formApi.resetForm();
  },
});
</script>

<template>
  <Modal class="w-[560px]" title="创建 MCP 密钥">
    <Form class="mx-4" />
  </Modal>
</template>
