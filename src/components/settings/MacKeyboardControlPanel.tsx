import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  BellRing,
  Keyboard,
  Loader2,
  MonitorCheck,
  RadioTower,
  RefreshCw,
  Sparkles,
  TestTube2,
} from "lucide-react";
import {
  macKeyboardApi,
  type MacKeyboardServicesStatus,
} from "@/lib/api/settings";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { ToggleRow } from "@/components/ui/toggle-row";

type Target =
  | "g610Listening"
  | "g610Blinking"
  | "codexDesktopWatcher"
  | "claudeRequestHooks"
  | "inputMapping"
  | "keyboardWriteMode"
  | "testBlink";
type SliderTarget =
  | "defaultBrightness"
  | "blinkBrightness"
  | "frequencyHz"
  | "burstSeconds"
  | "pauseSeconds";

const FALLBACK_DEFAULT_BRIGHTNESS = 50;

export function MacKeyboardControlPanel() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<MacKeyboardServicesStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [busyTarget, setBusyTarget] = useState<Target | SliderTarget | null>(
    null,
  );
  const [isDefaultBrightnessEnabled, setIsDefaultBrightnessEnabled] =
    useState(false);
  const [lastDefaultBrightness, setLastDefaultBrightness] = useState(
    FALLBACK_DEFAULT_BRIGHTNESS,
  );
  const [sliderValues, setSliderValues] = useState({
    defaultBrightness: FALLBACK_DEFAULT_BRIGHTNESS,
    blinkBrightness: 100,
    frequencyHz: 3,
    burstSeconds: 5,
    pauseSeconds: 15,
  });

  const loadStatus = useCallback(async () => {
    setIsLoading(true);
    try {
      setStatus(await macKeyboardApi.getStatus());
    } catch (error) {
      console.error("[MacKeyboardControlPanel] Failed to load status", error);
      toast.error(String(error));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  useEffect(() => {
    if (!status) return;
    const nextDefaultBrightness = status.defaultBrightness ?? 0;
    const defaultBrightnessEnabled = nextDefaultBrightness > 0;
    setIsDefaultBrightnessEnabled(defaultBrightnessEnabled);
    if (defaultBrightnessEnabled) {
      setLastDefaultBrightness(nextDefaultBrightness);
    }
    setSliderValues((current) => {
      return {
        defaultBrightness: defaultBrightnessEnabled
          ? nextDefaultBrightness
          : current.defaultBrightness > 0
            ? current.defaultBrightness
            : FALLBACK_DEFAULT_BRIGHTNESS,
        blinkBrightness: status.blinkBrightness ?? 100,
        frequencyHz: status.frequencyHz ?? 3,
        burstSeconds: status.burstSeconds ?? 5,
        pauseSeconds: status.pauseSeconds ?? 15,
      };
    });
  }, [status]);

  const setService = useCallback(
    async (target: Target, enabled: boolean) => {
      setBusyTarget(target);
      try {
        const next =
          target === "g610Listening"
            ? await macKeyboardApi.setG610Listening(enabled)
            : target === "g610Blinking"
              ? await macKeyboardApi.setG610Blinking(enabled)
              : target === "codexDesktopWatcher"
                ? await macKeyboardApi.setCodexDesktopWatcher(enabled)
                : target === "claudeRequestHooks"
                  ? await macKeyboardApi.setClaudeRequestHooks(enabled)
                  : await macKeyboardApi.setInputMapping(enabled);
        setStatus(next);
      } catch (error) {
        console.error(
          "[MacKeyboardControlPanel] Failed to toggle service",
          error,
        );
        toast.error(String(error));
        await loadStatus();
      } finally {
        setBusyTarget(null);
      }
    },
    [loadStatus],
  );

  const setSliderValue = useCallback(
    async (target: SliderTarget, value: number) => {
      const normalized =
        target === "defaultBrightness" || target === "blinkBrightness"
          ? Math.max(0, Math.min(100, Math.round(value)))
          : target === "frequencyHz"
            ? Math.max(0.5, Math.min(10, Math.round(value * 10) / 10))
            : target === "burstSeconds"
              ? Math.max(1, Math.min(60, Math.round(value)))
              : Math.max(0, Math.min(120, Math.round(value)));
      setBusyTarget(target);
      try {
        const next = await (target === "defaultBrightness"
          ? macKeyboardApi.setDefaultBrightness(normalized)
          : target === "blinkBrightness"
            ? macKeyboardApi.setBlinkBrightness(normalized)
            : target === "frequencyHz"
              ? macKeyboardApi.setFrequency(normalized)
              : target === "burstSeconds"
                ? macKeyboardApi.setBurstSeconds(normalized)
                : macKeyboardApi.setPauseSeconds(normalized));
        setStatus(next);
      } catch (error) {
        console.error("[MacKeyboardControlPanel] Failed to set value", error);
        toast.error(String(error));
        await loadStatus();
      } finally {
        setBusyTarget(null);
      }
    },
    [loadStatus],
  );

  const setDefaultBrightnessEnabled = useCallback(
    async (enabled: boolean) => {
      const nextBrightness = enabled
        ? Math.max(
            1,
            Math.round(sliderValues.defaultBrightness) ||
              lastDefaultBrightness ||
              FALLBACK_DEFAULT_BRIGHTNESS,
          )
        : 0;
      setIsDefaultBrightnessEnabled(enabled);
      setSliderValues((current) => ({
        ...current,
        defaultBrightness: enabled ? nextBrightness : current.defaultBrightness,
      }));
      if (enabled) {
        setLastDefaultBrightness(nextBrightness);
      }
      await setSliderValue("defaultBrightness", nextBrightness);
    },
    [lastDefaultBrightness, setSliderValue, sliderValues.defaultBrightness],
  );

  const setKeyboardWriteMode = useCallback(
    async (mode: string) => {
      setBusyTarget("keyboardWriteMode");
      try {
        setStatus(await macKeyboardApi.setKeyboardWriteMode(mode));
      } catch (error) {
        console.error(
          "[MacKeyboardControlPanel] Failed to set write mode",
          error,
        );
        toast.error(String(error));
        await loadStatus();
      } finally {
        setBusyTarget(null);
      }
    },
    [loadStatus],
  );

  const testBlink = useCallback(async () => {
    setBusyTarget("testBlink");
    try {
      setStatus(await macKeyboardApi.testBlink());
    } catch (error) {
      console.error("[MacKeyboardControlPanel] Failed to test blink", error);
      toast.error(String(error));
      await loadStatus();
    } finally {
      setBusyTarget(null);
    }
  }, [loadStatus]);

  const unavailableMessage = useMemo(() => {
    if (!status) return null;
    if (!status.supported) {
      return t("settings.advanced.macKeyboard.unsupported", {
        defaultValue: "Mac keyboard controls are only available on macOS.",
      });
    }
    const missing = [];
    if (!status.g610Listening.installed) missing.push("codex-g610-server-*");
    if (!status.inputMapping.installed) missing.push("codex-mac-input-*");
    return missing.length > 0
      ? t("settings.advanced.macKeyboard.missingScripts", {
          defaultValue: "Missing helper scripts: {{scripts}}",
          scripts: missing.join(", "),
        })
      : null;
  }, [status, t]);

  if (isLoading && !status) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        {t("common.loading", { defaultValue: "Loading..." })}
      </div>
    );
  }

  if (!status) return null;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-3 rounded-xl bg-muted/40 px-4 py-3 text-xs text-muted-foreground">
        <div className="min-w-0">
          <p className="font-medium text-foreground">
            {t("settings.advanced.macKeyboard.statusTitle", {
              defaultValue: "Current status",
            })}
          </p>
          <p className="break-words">
            {t("settings.advanced.macKeyboard.statusDetail", {
              defaultValue:
                "Listening: {{listening}} · Blinking: {{blinking}} · Claude: {{claude}} · Mapping: {{mapping}}",
              listening: status.g610Listening.status,
              blinking: status.g610Blinking.status,
              claude: status.claudeRequestHooks.status,
              mapping: status.inputMapping.status,
            })}
          </p>
          {unavailableMessage ? (
            <p className="mt-1 break-words text-amber-600 dark:text-amber-400">
              {unavailableMessage}
            </p>
          ) : null}
          <p className="mt-1 break-words">
            {t("settings.advanced.macKeyboard.deviceStatus", {
              defaultValue:
                "Device mode: {{mode}} · G610: {{g610}} · Apple: {{apple}}",
              mode: status.keyboardWriteMode || status.g610WriteMode,
              g610: status.g610LedAvailable ? "available" : "unavailable",
              apple: status.appleKbdAvailable ? "available" : "unavailable",
            })}
          </p>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          onClick={() => void loadStatus()}
          disabled={isLoading || busyTarget !== null}
          title={t("common.refresh", { defaultValue: "Refresh" })}
          aria-label={t("common.refresh", { defaultValue: "Refresh" })}
        >
          {isLoading ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <RefreshCw className="h-4 w-4" />
          )}
        </Button>
      </div>

      <div className="rounded-xl border border-border bg-card/50 p-4">
        <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <div className="min-w-0 space-y-1">
            <p className="text-sm font-medium leading-none">
              {t("settings.advanced.macKeyboard.writeMode", {
                defaultValue: "Controllable keyboard",
              })}
            </p>
            <p className="text-xs text-muted-foreground">
              {t("settings.advanced.macKeyboard.writeModeDescription", {
                defaultValue:
                  "Choose the brightness backend used by Claude hooks and the Codex Desktop watcher.",
              })}
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Select
              value={status.keyboardWriteMode || "auto"}
              onValueChange={(value) => void setKeyboardWriteMode(value)}
              disabled={busyTarget !== null || isLoading || !status.supported}
            >
              <SelectTrigger className="w-[220px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="auto">
                  {t("settings.advanced.macKeyboard.writeModeAuto", {
                    defaultValue: "Auto detect",
                  })}
                </SelectItem>
                <SelectItem
                  value="g610-led"
                  disabled={!status.g610LedAvailable}
                >
                  {t("settings.advanced.macKeyboard.writeModeG610", {
                    defaultValue: "Logitech G610",
                  })}
                </SelectItem>
                <SelectItem
                  value="apple-kbd"
                  disabled={!status.appleKbdAvailable}
                >
                  {t("settings.advanced.macKeyboard.writeModeApple", {
                    defaultValue: "Apple keyboard backlight",
                  })}
                </SelectItem>
              </SelectContent>
            </Select>
            <Button
              type="button"
              variant="outline"
              onClick={() => void testBlink()}
              disabled={
                busyTarget !== null ||
                isLoading ||
                !status.supported ||
                !status.g610Listening.installed
              }
            >
              {busyTarget === "testBlink" ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <TestTube2 className="mr-2 h-4 w-4" />
              )}
              {t("settings.advanced.macKeyboard.testBlink", {
                defaultValue: "Test blink",
              })}
            </Button>
          </div>
        </div>
        <div className="mt-3 space-y-1 text-xs text-muted-foreground">
          {status.keyboardDevices.map((device) => (
            <p key={device.id} className="break-words">
              {device.label}:{" "}
              {device.available
                ? t("settings.advanced.macKeyboard.available", {
                    defaultValue: "available",
                  })
                : t("settings.advanced.macKeyboard.unavailable", {
                    defaultValue: "unavailable",
                  })}
              {device.detail ? ` · ${device.detail}` : ""}
            </p>
          ))}
        </div>
      </div>

      <ToggleRow
        icon={<RadioTower className="h-4 w-4 text-blue-500" />}
        title={t("settings.advanced.macKeyboard.listening")}
        description={t("settings.advanced.macKeyboard.listeningDescription")}
        checked={status.g610Listening.running}
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed
        }
        onCheckedChange={(checked) => void setService("g610Listening", checked)}
      />

      <ToggleRow
        icon={<Sparkles className="h-4 w-4 text-amber-500" />}
        title={t("settings.advanced.macKeyboard.blinking")}
        description={t("settings.advanced.macKeyboard.blinkingDescription")}
        checked={status.g610Blinking.running}
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Blinking.installed
        }
        onCheckedChange={(checked) => void setService("g610Blinking", checked)}
      />

      <ToggleRow
        icon={<MonitorCheck className="h-4 w-4 text-violet-500" />}
        title={t("settings.advanced.macKeyboard.codexDesktopWatcher")}
        description={t(
          "settings.advanced.macKeyboard.codexDesktopWatcherDescription",
        )}
        checked={status.codexDesktopWatcher.running}
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed ||
          !status.codexDesktopWatcher.installed
        }
        onCheckedChange={(checked) =>
          void setService("codexDesktopWatcher", checked)
        }
      />

      <ToggleRow
        icon={<BellRing className="h-4 w-4 text-rose-500" />}
        title={t("settings.advanced.macKeyboard.claudeRequestHooks")}
        description={t(
          "settings.advanced.macKeyboard.claudeRequestHooksDescription",
        )}
        checked={status.claudeRequestHooks.running}
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed ||
          !status.claudeRequestHooks.installed
        }
        onCheckedChange={(checked) =>
          void setService("claudeRequestHooks", checked)
        }
      />

      <RangeSlider
        label={t("settings.advanced.macKeyboard.defaultBrightness")}
        description={t(
          "settings.advanced.macKeyboard.defaultBrightnessDescription",
        )}
        enabled={isDefaultBrightnessEnabled}
        onEnabledChange={(enabled) => void setDefaultBrightnessEnabled(enabled)}
        value={sliderValues.defaultBrightness}
        min={0}
        max={100}
        step={1}
        suffix="%"
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed
        }
        onChange={(value) => {
          if (value > 0) {
            setLastDefaultBrightness(value);
          }
          setSliderValues((current) => ({
            ...current,
            defaultBrightness: value,
          }));
        }}
        onCommit={(value) => void setSliderValue("defaultBrightness", value)}
      />

      <RangeSlider
        label={t("settings.advanced.macKeyboard.blinkBrightness")}
        description={t(
          "settings.advanced.macKeyboard.blinkBrightnessDescription",
        )}
        value={sliderValues.blinkBrightness}
        min={0}
        max={100}
        step={1}
        suffix="%"
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed
        }
        onChange={(value) =>
          setSliderValues((current) => ({
            ...current,
            blinkBrightness: value,
          }))
        }
        onCommit={(value) => void setSliderValue("blinkBrightness", value)}
      />

      <RangeSlider
        label={t("settings.advanced.macKeyboard.frequency")}
        description={t("settings.advanced.macKeyboard.frequencyDescription")}
        value={sliderValues.frequencyHz}
        min={0.5}
        max={10}
        step={0.5}
        suffix=" Hz"
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed
        }
        onChange={(value) =>
          setSliderValues((current) => ({
            ...current,
            frequencyHz: value,
          }))
        }
        onCommit={(value) => void setSliderValue("frequencyHz", value)}
      />

      <RangeSlider
        label={t("settings.advanced.macKeyboard.burstSeconds")}
        description={t("settings.advanced.macKeyboard.burstSecondsDescription")}
        value={sliderValues.burstSeconds}
        min={1}
        max={60}
        step={1}
        suffix="s"
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed
        }
        onChange={(value) =>
          setSliderValues((current) => ({
            ...current,
            burstSeconds: value,
          }))
        }
        onCommit={(value) => void setSliderValue("burstSeconds", value)}
      />

      <RangeSlider
        label={t("settings.advanced.macKeyboard.pauseSeconds")}
        description={t("settings.advanced.macKeyboard.pauseSecondsDescription")}
        value={sliderValues.pauseSeconds}
        min={0}
        max={120}
        step={1}
        suffix="s"
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.g610Listening.installed
        }
        onChange={(value) =>
          setSliderValues((current) => ({
            ...current,
            pauseSeconds: value,
          }))
        }
        onCommit={(value) => void setSliderValue("pauseSeconds", value)}
      />

      <ToggleRow
        icon={<Keyboard className="h-4 w-4 text-emerald-500" />}
        title={t("settings.advanced.macKeyboard.inputMapping")}
        description={t("settings.advanced.macKeyboard.inputMappingDescription")}
        checked={status.inputMapping.running}
        disabled={
          busyTarget !== null ||
          isLoading ||
          !status.supported ||
          !status.inputMapping.installed
        }
        onCheckedChange={(checked) => void setService("inputMapping", checked)}
      />
    </div>
  );
}

interface RangeSliderProps {
  label: string;
  description: string;
  value: number;
  min: number;
  max: number;
  step: number;
  suffix: string;
  disabled: boolean;
  enabled?: boolean;
  onEnabledChange?: (enabled: boolean) => void;
  onChange: (value: number) => void;
  onCommit: (value: number) => void;
}

function RangeSlider({
  label,
  description,
  value,
  min,
  max,
  step,
  suffix,
  disabled,
  enabled,
  onEnabledChange,
  onChange,
  onCommit,
}: RangeSliderProps) {
  const displayValue = step < 1 ? value.toFixed(1) : String(Math.round(value));
  const sliderDisabled = disabled || enabled === false;

  return (
    <div className="rounded-xl border border-border bg-card/50 p-4 transition-colors hover:bg-muted/50">
      <div className="flex items-center justify-between gap-4">
        <div className="space-y-1">
          <p className="text-sm font-medium leading-none">{label}</p>
          <p className="text-xs text-muted-foreground">{description}</p>
        </div>
        <div className="flex shrink-0 items-center gap-3">
          <span className="w-16 text-right text-sm tabular-nums text-muted-foreground">
            {displayValue}
            {suffix}
          </span>
          {typeof enabled === "boolean" && onEnabledChange ? (
            <Switch
              checked={enabled}
              onCheckedChange={onEnabledChange}
              disabled={disabled}
              aria-label={label}
            />
          ) : null}
        </div>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={sliderDisabled}
        aria-label={label}
        className="mt-4 h-2 w-full cursor-pointer accent-emerald-500 disabled:cursor-not-allowed disabled:opacity-50"
        onChange={(event) => onChange(Number(event.currentTarget.value))}
        onPointerUp={(event) => onCommit(Number(event.currentTarget.value))}
        onKeyUp={(event) => onCommit(Number(event.currentTarget.value))}
        onBlur={(event) => onCommit(Number(event.currentTarget.value))}
      />
    </div>
  );
}
