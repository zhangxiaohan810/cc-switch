import { useEffect, useState, useCallback } from "react";
import type { ProviderCategory } from "@/types";
import {
  getApiKeyFromConfig,
  setApiKeyInConfig,
  hasApiKeyField,
} from "@/utils/providerConfigUtils";

interface UseApiKeyStateProps {
  initialConfig?: string;
  onConfigChange: (config: string) => void;
  selectedPresetId: string | null;
  category?: ProviderCategory;
  appType?: string;
  apiKeyField?: string;
}

/**
 * 管理 API Key 输入状态
 * 自动同步 API Key 和 JSON 配置
 */
export function useApiKeyState({
  initialConfig,
  onConfigChange,
  selectedPresetId,
  category,
  appType,
  apiKeyField,
}: UseApiKeyStateProps) {
  const [apiKey, setApiKey] = useState(() => {
    if (initialConfig) {
      return getApiKeyFromConfig(initialConfig, appType);
    }
    return "";
  });

  // 当外部通过 form.reset / 读取 live 等方式更新配置时，同步回 API Key 状态
  // - 仅在 JSON 可解析时同步，避免用户编辑 JSON 过程中因临时无效导致输入框闪烁
  useEffect(() => {
    if (!initialConfig) return;

    try {
      JSON.parse(initialConfig);
    } catch {
      return;
    }

    // 从配置中提取 API Key（如果不存在则返回空字符串）
    const extracted = getApiKeyFromConfig(initialConfig, appType);
    if (extracted !== apiKey) {
      setApiKey(extracted);
    }
  }, [initialConfig, appType, apiKey]);

  const handleApiKeyChange = useCallback(
    (key: string) => {
      setApiKey(key);

      const configString = setApiKeyInConfig(
        initialConfig || "{}",
        key.trim(),
        {
          // 非官方供应商显示 API Key 输入框时，应确保输入能落入配置。
          // 编辑旧配置时可能缺少 env key，这里也允许补齐。
          createIfMissing:
            category !== undefined &&
            category !== "official" &&
            category !== "cloud_provider",
          appType,
          apiKeyField,
        },
      );

      onConfigChange(configString);
    },
    [
      initialConfig,
      selectedPresetId,
      category,
      appType,
      apiKeyField,
      onConfigChange,
    ],
  );

  const showApiKey = useCallback(
    (config: string, isEditMode: boolean) => {
      return (
        selectedPresetId !== null ||
        (isEditMode && hasApiKeyField(config, appType))
      );
    },
    [selectedPresetId, appType],
  );

  return {
    apiKey,
    setApiKey,
    handleApiKeyChange,
    showApiKey,
  };
}
