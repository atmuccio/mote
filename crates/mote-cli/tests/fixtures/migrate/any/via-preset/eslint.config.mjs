// typescript-eslint's strict configuration turns `@typescript-eslint/no-explicit-any` on; nothing
// below turns it off, except for the generated clients.
import js from '@eslint/js';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['dist/'] },
  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  ...tseslint.configs.stylistic,
  {
    rules: { 'no-console': 'warn' },
  },
  {
    files: ['src/generated/**/*.ts'],
    rules: { '@typescript-eslint/no-explicit-any': 'off' },
  },
);
