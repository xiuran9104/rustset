import { computed, ref, type Ref } from 'vue';

export function useLocalTableSearch<T extends Record<string, any>>(
  rows: Ref<T[]>,
  keys: (keyof T)[],
) {
  const searchText = ref('');
  const filteredRows = computed(() => {
    const keyword = searchText.value.trim().toLowerCase();
    if (!keyword) return rows.value;
    return rows.value.filter((row) =>
      keys.some((key) => String(row[key] ?? '').toLowerCase().includes(keyword)),
    );
  });
  return { filteredRows, searchText };
}
