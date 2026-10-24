import aiStudioLogo from "../assets/aistudio.svg";
import anthropicLogo from "../assets/anthropic.svg";
import baiduCloudLogo from "../assets/baiducloud-color.svg";
import claudeLogo from "../assets/claude-color.svg";
import deepseekLogo from "../assets/deepseek-color.svg";
import doubaoLogo from "../assets/doubao-color.svg";
import geminiLogo from "../assets/gemini-color.svg";
import grokLogo from "../assets/grok.svg";
import hunyuanLogo from "../assets/hunyuan-color.svg";
import kimiLogo from "../assets/kimi-color.svg";
import metaLogo from "../assets/meta-color.svg";
import minimaxLogo from "../assets/minimax-color.svg";
import openaiLogo from "../assets/openai.svg";
import opencodeLogo from "../assets/opencode.svg";
import openRouterLogo from "../assets/openrouter-color.svg";
import qwenLogo from "../assets/qwen-color.svg";
import zhipuLogo from "../assets/zhipu-color.svg";
import xiaomiMimoLogo from "../assets/xiaomimimo.svg";
import type { BuiltinProviderOption } from "../types/domain";
import { isChineseLocale, type AppLocale } from "../i18n/locale";

import catalog from "../../crates/key-switch-core/provider-catalog.json";
const logos: Record<string, string> = {openai: openaiLogo, claude: claudeLogo, gemini: geminiLogo, deepseek: deepseekLogo, opencode: opencodeLogo, qwen: qwenLogo, kimi: kimiLogo, zhipu: zhipuLogo, mimo: xiaomiMimoLogo, grok: grokLogo, meta: metaLogo, openrouter: openRouterLogo, minimax: minimaxLogo, doubao: doubaoLogo, hunyuan: hunyuanLogo, qianfan: baiduCloudLogo, anthropic: anthropicLogo, aistudio: aiStudioLogo};
export const builtinProviderCatalog: BuiltinProviderOption[] = catalog.map(provider => ({ ...provider, tone: provider.tone as BuiltinProviderOption["tone"], logo: logos[provider.id] }));

export function getBuiltinProviderName(provider: BuiltinProviderOption, locale: AppLocale): string {
  return isChineseLocale(locale) ? provider.nameZh : provider.nameEn;
}
