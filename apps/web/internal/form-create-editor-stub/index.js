// wangeditor 已弃更且存在未修复的 XSS（GHSA-g7mw-5cq6-fv82），应用侧
// 富文本统一使用 Tinymce；此占位组件仅为满足设计器的默认导入，不会渲染。
const FcEditor = {
  name: 'FcEditor',
  formCreateParser: { label: '富文本' },
};

export default FcEditor;
