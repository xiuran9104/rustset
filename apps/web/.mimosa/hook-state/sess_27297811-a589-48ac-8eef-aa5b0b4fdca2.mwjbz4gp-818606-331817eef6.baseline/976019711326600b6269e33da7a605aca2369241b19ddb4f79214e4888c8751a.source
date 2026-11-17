import type { RouteRecordRaw } from 'vue-router';

const routes: RouteRecordRaw[] = [
  {
    path: '/ai',
    name: 'Ai',
    meta: {
      title: 'AI 大模型',
      icon: 'tabler:ai',
      order: 30,
    },
    children: [
      {
        path: 'chat',
        component: () => import('#/views/ai/chat/index/index.vue'),
        name: 'AiChat',
        meta: {
          title: 'AI 对话',
          icon: 'lucide:message-circle',
        },
      },
      {
        path: 'chat/manager',
        component: () => import('#/views/ai/chat/manager/index.vue'),
        name: 'AiChatManager',
        meta: {
          title: '对话管理',
          icon: 'lucide:messages-square',
        },
      },
      {
        path: 'image',
        component: () => import('#/views/ai/image/index/index.vue'),
        name: 'AiImage',
        meta: {
          title: 'AI 绘图',
          icon: 'lucide:image',
        },
      },
      {
        path: 'image/manager',
        component: () => import('#/views/ai/image/manager/index.vue'),
        name: 'AiImageManager',
        meta: {
          title: '绘图管理',
          icon: 'lucide:images',
        },
      },
      {
        path: 'write',
        component: () => import('#/views/ai/write/index/index.vue'),
        name: 'AiWrite',
        meta: {
          title: 'AI 写作',
          icon: 'lucide:pen-line',
        },
      },
      {
        path: 'write/manager',
        component: () => import('#/views/ai/write/manager/index.vue'),
        name: 'AiWriteManager',
        meta: {
          title: '写作管理',
          icon: 'lucide:book-text',
        },
      },
      {
        path: 'music',
        component: () => import('#/views/ai/music/index/index.vue'),
        name: 'AiMusic',
        meta: {
          title: 'AI 音乐',
          icon: 'lucide:music',
        },
      },
      {
        path: 'music/manager',
        component: () => import('#/views/ai/music/manager/index.vue'),
        name: 'AiMusicManager',
        meta: {
          title: '音乐管理',
          icon: 'lucide:list-music',
        },
      },
      {
        path: 'knowledge',
        component: () => import('#/views/ai/knowledge/knowledge/index.vue'),
        name: 'AiKnowledge',
        meta: {
          title: '知识库',
          icon: 'lucide:database',
        },
      },
      {
        path: 'model',
        component: () => import('#/views/ai/model/model/index.vue'),
        name: 'AiModel',
        meta: {
          title: '模型管理',
          icon: 'lucide:brain-circuit',
        },
      },
      {
        path: 'model/chat-role',
        component: () => import('#/views/ai/model/chatRole/index.vue'),
        name: 'AiModelChatRole',
        meta: {
          title: '聊天角色',
          icon: 'lucide:bot',
        },
      },
      {
        path: 'model/tool',
        component: () => import('#/views/ai/model/tool/index.vue'),
        name: 'AiModelTool',
        meta: {
          title: '工具管理',
          icon: 'lucide:wrench',
        },
      },
      {
        path: 'image/square',
        component: () => import('#/views/ai/image/square/index.vue'),
        name: 'AiImageSquare',
        meta: {
          noCache: true,
          hidden: true,
          canTo: true,
          title: '绘图作品',
          icon: 'lucide:images',
          activePath: '/ai/image',
        },
      },
      {
        path: 'knowledge/document',
        component: () => import('#/views/ai/knowledge/document/index.vue'),
        name: 'AiKnowledgeDocument',
        meta: {
          noCache: true,
          hidden: true,
          canTo: true,
          title: '知识库文档',
          icon: 'lucide:files',
          activePath: '/ai/knowledge',
        },
      },
      {
        path: 'knowledge/document/create',
        component: () => import('#/views/ai/knowledge/document/form/index.vue'),
        name: 'AiKnowledgeDocumentCreate',
        meta: {
          noCache: true,
          hidden: true,
          canTo: true,
          title: '创建文档',
          icon: 'lucide:file-plus-2',
          activePath: '/ai/knowledge',
        },
      },
      {
        path: 'knowledge/document/update',
        component: () => import('#/views/ai/knowledge/document/form/index.vue'),
        name: 'AiKnowledgeDocumentUpdate',
        meta: {
          noCache: true,
          hidden: true,
          canTo: true,
          title: '修改文档',
          icon: 'lucide:file-pen-line',
          activePath: '/ai/knowledge',
        },
      },
      {
        path: 'knowledge/retrieval',
        component: () =>
          import('#/views/ai/knowledge/knowledge/retrieval/index.vue'),
        name: 'AiKnowledgeRetrieval',
        meta: {
          noCache: true,
          hidden: true,
          canTo: true,
          title: '文档召回测试',
          icon: 'lucide:search-check',
          activePath: '/ai/knowledge',
        },
      },
      {
        path: 'knowledge/segment',
        component: () => import('#/views/ai/knowledge/segment/index.vue'),
        name: 'AiKnowledgeSegment',
        meta: {
          noCache: true,
          hidden: true,
          canTo: true,
          title: '知识库分段',
          icon: 'lucide:blocks',
          activePath: '/ai/knowledge',
        },
      },
    ],
  },
];

export default routes;
