import { ref, onMounted, onActivated, computed } from 'vue';
import { message } from 'ant-design-vue';

export interface CrudApi<T> {
  list: () => Promise<any>;
  create?: (data: T) => Promise<any>;
  update?: (id: number, data: T) => Promise<any>;
  del?: (id: number) => Promise<any>;
}

export interface UseCrudListOptions<T> {
  api: CrudApi<T>;
  defaultForm: () => T;
  searchKeys?: (keyof T)[];
  onCreated?: () => void;
  onUpdated?: () => void;
  onDeleted?: () => void;
}

export function useCrudList<T extends Record<string, any>>(options: UseCrudListOptions<T>) {
  const { api, defaultForm, searchKeys = [] } = options;

  const data = ref<T[]>([]);
  const loading = ref(false);
  const modalVisible = ref(false);
  const editingId = ref<number | undefined>();
  const searchText = ref('');
  const form = ref<T>(defaultForm()) as ReturnType<typeof ref<T>>;

  const filtered = computed(() => {
    if (!searchText.value.trim() || searchKeys.length === 0) return data.value;
    const q = searchText.value.toLowerCase();
    return data.value.filter((item) =>
      searchKeys.some((k) => {
        const v = (item as T)[k];
        return v != null && String(v).toLowerCase().includes(q);
      }),
    );
  });

  async function fetchData() {
    loading.value = true;
    try {
      const result = await api.list();
      data.value = (Array.isArray(result) ? result : result?.data ?? []) as T[];
    } catch {
      message.error('加载失败');
    } finally {
      loading.value = false;
    }
  }

  onMounted(fetchData);
  // keep-alive 标签页切回时刷新，其他页面增改数据后本页列表不会过期
  onActivated(fetchData);

  function openCreate() {
    editingId.value = undefined;
    form.value = defaultForm();
    modalVisible.value = true;
  }

  function openEdit(row: T) {
    editingId.value = (row as any).id;
    form.value = { ...row };
    modalVisible.value = true;
  }

  async function handleSubmit() {
    try {
      if (editingId.value) {
        if (api.update) await api.update(editingId.value, form.value as any);
        message.success('更新成功');
        options.onUpdated?.();
      } else {
        if (api.create) await api.create(form.value as any);
        message.success('创建成功');
        options.onCreated?.();
      }
      modalVisible.value = false;
      await fetchData();
    } catch {
      message.error('保存失败');
    }
  }

  async function handleDelete(id: any) {
    try {
      if (api.del) await api.del(id);
      message.success('删除成功');
      options.onDeleted?.();
      await fetchData();
    } catch {
      message.error('删除失败');
    }
  }

  return {
    data,
    loading,
    modalVisible,
    editingId,
    searchText,
    form,
    filtered,
    fetchData,
    openCreate,
    openEdit,
    handleSubmit,
    handleDelete,
  };
}
