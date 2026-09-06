// 全局环境声明：仅补充 Window 上由宿主/脚本注入的自定义成员。
// （constants/sprite/renderer 等顶层变量是脚本间共享的全局词法标识，由各自的源文件声明，
//  无需在此重复声明。）

interface Window {
  /** 上游 src/shared 纯逻辑的 IIFE 构建产物（shared-core.js 注入） */
  PetShared: any;
  /** 渲染端调试/遥测状态 */
  __dshPetDebug?: {
    errors: string[];
    configOk: boolean;
    spriteCount: number;
    bootAt: number;
    [k: string]: any;
  };
  /** 本窗口宠物 id（renderer boot 后写入；bridge 事件订阅用） */
  __petId?: string;
  /** bridge 通道（sprite 交互/事件原语，见 ts/bridge.ts） */
  petBridge?: any;
  /** 立即上报遥测（bridge.ts 注入） */
  __reportTele?: (data: any) => void;
  __teleTimer?: any;
  /** CDP 冒烟工具用（可选） */
  __sprite?: any;
}
