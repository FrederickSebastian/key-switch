<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { computed } from "vue";
import { builtinProviderCatalog } from "../data/providerCatalog";
import type { ProviderSummary } from "../types/domain";

const props = defineProps<{ provider: Pick<ProviderSummary, "name" | "abbreviation" | "tone" | "logo" | "kind"> & { id?: string } }>();
const { t } = useI18n();
const logo = computed(() => props.provider.logo || (props.provider.kind === "builtin"
  ? builtinProviderCatalog.find(provider => provider.id === props.provider.id)?.logo
  : undefined));
</script>

<template>
  <span
    class="provider-avatar"
    :class="[`provider-avatar--${provider.tone}`, { 'provider-avatar--image': logo }]"
  >
    <img v-if="logo" :class="{ 'provider-avatar__custom-image': provider.kind === 'custom' }" :src="logo" :alt="t('common.providerIconAlt', { name: provider.name })" />
    <span v-else class="provider-abbr">{{ provider.abbreviation }}</span>
  </span>
</template>

<style scoped>
.provider-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 38px;
  height: 38px;
  flex: 0 0 38px;
  border-radius: 10px;
  background: transparent;
  border: none;
  box-sizing: border-box;
  overflow: hidden;
}

.provider-avatar img {
  width: 22px;
  height: 22px;
  max-width: 22px;
  max-height: 22px;
  object-fit: contain;
  display: block;
  margin: auto;
}

.provider-avatar img.provider-avatar__custom-image {
  width: 100%;
  height: 100%;
  max-width: none;
  max-height: none;
  object-fit: cover;
}

:global(:root[data-theme="dark"] .provider-avatar--image) {
  background: rgba(255, 255, 255, 0.92);
}

.provider-abbr {
  font-size: 13px;
  font-weight: 700;
  color: var(--text-secondary);
  line-height: 1;
}

</style>
