import type { ZodType } from 'zod';

/**
 * vee-validate 的类型化 schema 接口适配器。
 *
 * 替代已移除的 `@vee-validate/zod`（其最新版仍锁定 zod ^3，无法随 zod 4
 * 升级修复 GHSA-mrhx-c7xc-m3w5）。vee-validate 约定 parse 返回
 * `{ errors: { path, errors }[], value }`，这里直接用 zod 自身的
 * safeParseAsync 实现，语义与官方适配器一致。
 */
export function toTypedSchema(schema: ZodType) {
  return {
    async parse(value: unknown) {
      const result = await schema.safeParseAsync(value);
      if (result.success) {
        return { errors: [], value: result.data };
      }
      const collected = new Map<string, string[]>();
      for (const issue of result.error.issues) {
        const path = issue.path.map(String).join('.');
        const messages = collected.get(path) ?? [];
        messages.push(issue.message);
        collected.set(path, messages);
      }
      return {
        errors: [...collected.entries()].map(([path, errors]) => ({
          path,
          errors,
        })),
        value,
      };
    },
  };
}
