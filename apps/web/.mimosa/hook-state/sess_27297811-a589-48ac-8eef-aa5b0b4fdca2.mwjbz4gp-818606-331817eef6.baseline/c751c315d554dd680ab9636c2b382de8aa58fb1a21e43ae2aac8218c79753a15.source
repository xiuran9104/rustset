import { describe, expect, it } from 'vitest';

import { hasRequiredQuery } from './route-context';

describe('hasRequiredQuery', () => {
  it('accepts all required route parameters', () => {
    expect(
      hasRequiredQuery({ id: '12', knowledgeId: '3' }, ['knowledgeId', 'id']),
    ).toBe(true);
  });

  it('rejects missing and empty route parameters', () => {
    expect(hasRequiredQuery({ knowledgeId: '' }, ['knowledgeId'])).toBe(false);
    expect(hasRequiredQuery({}, ['documentId'])).toBe(false);
  });
});
