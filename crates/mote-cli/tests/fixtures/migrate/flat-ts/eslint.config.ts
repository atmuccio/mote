// A TypeScript flat configuration: type annotations, `as` and `satisfies` are read through.
import type { Linter } from 'eslint';
import tseslint from 'typescript-eslint';

const base = {
  rules: {
    'no-debugger': 'error',
  },
} satisfies Linter.Config;

export default tseslint.config(
  { ignores: ['coverage/**'] },
  base,
  {
    files: ['src/**/*.ts'] as string[],
    rules: { '@typescript-eslint/no-explicit-any': 'warn' } as Linter.RulesRecord,
  },
  tseslint.configs.stylisticTypeChecked,
) satisfies Linter.Config[];
