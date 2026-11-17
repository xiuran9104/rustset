// wangEditor 5 版 fcEditor，替代已弃更的 @form-create/component-wangeditor
// （v4 封装，GHSA-g7mw-5cq6-fv82 XSS 无修复版）。
// 导出契约与 v4 一致：default export 组件，props 为
// modelValue / disabled / config / init，v-model 同步 HTML。
import '@wangeditor/editor/dist/css/style.css';

import { Editor, Toolbar } from '@wangeditor/editor-for-vue';
import { defineComponent, h, shallowRef, watch } from 'vue';

export default defineComponent({
  name: 'fcEditor',
  props: {
    modelValue: String,
    init: Function,
    disabled: Boolean,
    config: Object,
  },
  inheritAttrs: false,
  emits: ['update:modelValue'],
  setup(props, { attrs, emit }) {
    const editorRef = shallowRef(null);
    const toolbarConfig = {};
    const editorConfig = {
      placeholder: '请输入内容...',
      ...(props.config || {}),
    };

    const handleCreated = (editor) => {
      editorRef.value = editor;
      if (props.disabled) {
        editor.disable();
      }
      props.init?.(editor);
    };

    watch(
      () => props.disabled,
      (value) => {
        const editor = editorRef.value;
        if (!editor) return;
        value ? editor.disable() : editor.enable();
      },
    );

    return () =>
      h(
        'div',
        {
          ...attrs,
          style:
            'border:1px solid #d9d9d9;border-radius:6px;z-index:100;line-height:normal;overflow:hidden;',
        },
        [
          h(Toolbar, {
            editor: editorRef.value,
            defaultConfig: toolbarConfig,
            mode: 'default',
            style: 'border-bottom:1px solid #d9d9d9;',
          }),
          h(Editor, {
            modelValue: props.modelValue || '<p><br></p>',
            'onUpdate:modelValue': (value) => emit('update:modelValue', value),
            defaultConfig: editorConfig,
            mode: 'default',
            onCreated: handleCreated,
            style: 'height:350px;overflow-y:hidden;',
          }),
        ],
      );
  },
});
