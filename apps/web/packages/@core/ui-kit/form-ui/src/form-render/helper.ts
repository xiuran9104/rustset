import type { ZodObject, ZodType } from 'zod';

import { isObject, isString } from '@vben-core/shared/utils';

/**
 * Get the lowest level Zod type.
 * This will unpack optionals, refinements, etc.
 *
 * zod 4 removed `_def.typeName` / `ZodEffects`; unwrapping now uses the
 * public `unwrap()` (optional/default/nullable) and `.schema` shapes.
 */
export function getBaseRules<ChildType extends ZodObject | ZodType = ZodType>(
  schema: ChildType | ZodType,
): ChildType | null {
  if (!schema || isString(schema)) return null;
  const anySchema = schema as any;
  if (typeof anySchema.unwrap === 'function') {
    return getBaseRules(anySchema.unwrap() as ChildType);
  }
  if (anySchema.schema) {
    return getBaseRules(anySchema.schema as ChildType);
  }
  return schema as ChildType;
}

/**
 * Search for a "ZodDefault" in the Zod stack and return its value.
 */
export function getDefaultValueInZodStack(schema: ZodType): any {
  if (!schema || isString(schema)) {
    return;
  }
  const anySchema = schema as any;
  if (
    typeof anySchema.defaultValue === 'function' &&
    typeof anySchema.unwrap === 'function'
  ) {
    return anySchema.defaultValue();
  }
  if (typeof anySchema.unwrap === 'function') {
    return getDefaultValueInZodStack(anySchema.unwrap());
  }
  if (anySchema.schema) {
    return getDefaultValueInZodStack(anySchema.schema);
  }
}

export function isEventObjectLike(obj: any) {
  if (!obj || !isObject(obj)) {
    return false;
  }
  return Reflect.has(obj, 'target') && Reflect.has(obj, 'stopPropagation');
}
