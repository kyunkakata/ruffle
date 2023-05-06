import type { BaseLoadOptions } from "./load-options";
import {
    AutoPlay,
    UnmuteOverlay,
    WindowMode,
    Letterbox,
    LogLevel,
} from "./load-options";

export const DEFAULT_CONFIG: Required<BaseLoadOptions> = {
    allowScriptAccess: false,
    parameters: {},
    autoplay: AutoPlay.Auto,
    backgroundColor: null,
    letterbox: Letterbox.Fullscreen,
    unmuteOverlay: UnmuteOverlay.Visible,
    upgradeToHttps: true,
    compatibilityRules: true,
    warnOnUnsupportedContent: true,
    logLevel: LogLevel.Error,
    showSwfDownload: false,
    contextMenu: true,
    // Backwards-compatibility option
    preloader: true,
    splashScreen: true,
    maxExecutionDuration: 15,
    base: null,
    menu: true,
    salign: "",
    quality: "high",
    scale: "showAll",
    forceScale: false,
    frameRate: null,
    wmode: WindowMode.Opaque,
    publicPath: null,
    polyfills: true,
    playerVersion: null,
    preferredRenderer: null,
};
