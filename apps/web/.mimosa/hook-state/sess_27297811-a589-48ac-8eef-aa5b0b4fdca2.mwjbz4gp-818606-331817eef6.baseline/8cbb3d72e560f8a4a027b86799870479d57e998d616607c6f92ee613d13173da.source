import type {
  LocationQuery,
  RouteLocationRaw,
  Router,
} from 'vue-router';

import { message } from 'ant-design-vue';

const KNOWLEDGE_HOME: RouteLocationRaw = { path: '/ai/knowledge' };

function hasRequiredQuery(query: LocationQuery, keys: string[]): boolean {
  return keys.every((key) => {
    const value = query[key];
    return Array.isArray(value)
      ? value.some((item) => item !== null && item !== '')
      : value !== null && value !== undefined && value !== '';
  });
}

async function ensureKnowledgeRouteContext(options: {
  errorMessage: string;
  fallback?: RouteLocationRaw;
  query: LocationQuery;
  required: string[];
  router: Router;
}): Promise<boolean> {
  if (hasRequiredQuery(options.query, options.required)) {
    return true;
  }

  message.error(options.errorMessage);
  await options.router.replace(options.fallback ?? KNOWLEDGE_HOME);
  return false;
}

export { ensureKnowledgeRouteContext, hasRequiredQuery, KNOWLEDGE_HOME };
