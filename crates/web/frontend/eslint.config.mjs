import pluginVue from "eslint-plugin-vue";
import eslintConfigPrettier from "@vue/eslint-config-prettier";
import {
  configureVueProject,
  defineConfigWithVueTs,
  vueTsConfigs,
} from "@vue/eslint-config-typescript";

configureVueProject({
  rootDir: import.meta.dirname,
  tsSyntaxInTemplates: true,
});

export default defineConfigWithVueTs(
  {
    ignores: [
      "dist/**",
      "node_modules/**",
      "src/node_modules/**",
      "src/auto-imports.d.ts",
      "src/components.d.ts",
      ".DS_Store",
    ],
  },
  pluginVue.configs["flat/essential"],
  vueTsConfigs.recommended,
  eslintConfigPrettier,
  {
    files: ["**/*.{ts,tsx,vue,mts,cts,js,jsx,mjs,cjs}"],
    rules: {
      "vue/multi-word-component-names": "off",
      "@typescript-eslint/no-explicit-any": "off",
      "@typescript-eslint/no-unused-vars": [
        "warn",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
          caughtErrorsIgnorePattern: "^_",
        },
      ],
    },
  },
);
