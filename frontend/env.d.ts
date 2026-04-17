// 讓 TS 認識 CSS 檔案
declare module "*.css" {
    const content: { [className: string]: string };
    export default content;
}