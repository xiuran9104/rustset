import type { RouteRecordRaw } from 'vue-router';

const routes: RouteRecordRaw[] = [
  {
    path: '/system',
    name: 'System',
    meta: {
      title: '系统功能',
      icon: 'lucide:settings',
      order: 10,
    },
    children: [
      {
        path: '/system/user',
        component: () => import('#/views/system/user/index.vue'),
        name: 'SystemUser',
        meta: {
          title: '用户管理',
          icon: 'lucide:users',
        },
      },
      {
        path: '/system/role',
        component: () => import('#/views/system/role/index.vue'),
        name: 'SystemRole',
        meta: {
          title: '角色管理',
          icon: 'lucide:user-cog',
        },
      },
      {
        path: '/system/menu',
        component: () => import('#/views/system/menu/index.vue'),
        name: 'SystemMenu',
        meta: {
          title: '菜单管理',
          icon: 'lucide:list-tree',
        },
      },
      {
        path: '/system/dept',
        component: () => import('#/views/system/dept/index.vue'),
        name: 'SystemDept',
        meta: {
          title: '部门管理',
          icon: 'lucide:network',
        },
      },
      {
        path: '/system/post',
        component: () => import('#/views/system/post/index.vue'),
        name: 'SystemPost',
        meta: {
          title: '岗位管理',
          icon: 'lucide:briefcase-business',
        },
      },
      {
        path: '/system/dict',
        component: () => import('#/views/system/dict/index.vue'),
        name: 'SystemDict',
        meta: {
          title: '字典管理',
          icon: 'lucide:book-open',
        },
      },
      {
        path: '/system/notice',
        component: () => import('#/views/system/notice/index.vue'),
        name: 'SystemNotice',
        meta: {
          title: '通知公告',
          icon: 'lucide:megaphone',
        },
      },
      {
        path: '/system/operatelog',
        component: () => import('#/views/system/operatelog/index.vue'),
        name: 'SystemOperateLog',
        meta: {
          title: '操作日志',
          icon: 'lucide:file-clock',
        },
      },
      {
        path: '/system/loginlog',
        component: () => import('#/views/system/loginlog/index.vue'),
        name: 'SystemLoginLog',
        meta: {
          title: '登录日志',
          icon: 'lucide:log-in',
        },
      },
    ],
  },
  {
    path: '/system/basic',
    name: 'SystemBasic',
    meta: {
      title: '基础设置',
      icon: 'lucide:sliders-horizontal',
      order: 11,
    },
    children: [
      {
        path: '/system/tenant',
        component: () => import('#/views/system/tenant/index.vue'),
        name: 'SystemTenant',
        meta: {
          title: '租户管理',
          icon: 'lucide:building-2',
        },
      },
      {
        path: '/system/tenant-package',
        component: () => import('#/views/system/tenantPackage/index.vue'),
        name: 'SystemTenantPackage',
        meta: {
          title: '租户套餐',
          icon: 'lucide:package',
        },
      },
      {
        path: '/system/area',
        component: () => import('#/views/system/area/index.vue'),
        name: 'SystemArea',
        meta: {
          title: '地区管理',
          icon: 'lucide:map',
        },
      },
      {
        path: '/system/social-client',
        component: () => import('#/views/system/social/client/index.vue'),
        name: 'SystemSocialClient',
        meta: {
          title: '社交客户端',
          icon: 'lucide:share-2',
        },
      },
      {
        path: '/system/social-user',
        component: () => import('#/views/system/social/user/index.vue'),
        name: 'SystemSocialUser',
        meta: {
          title: '社交用户',
          icon: 'lucide:users-round',
        },
      },
      {
        path: '/system/oauth2-client',
        component: () => import('#/views/system/oauth2/client/index.vue'),
        name: 'SystemOauth2Client',
        meta: {
          title: 'OAuth2 客户端',
          icon: 'lucide:key-round',
        },
      },
      {
        path: '/system/oauth2-token',
        component: () => import('#/views/system/oauth2/token/index.vue'),
        name: 'SystemOauth2Token',
        meta: {
          title: 'OAuth2 令牌',
          icon: 'lucide:ticket-check',
        },
      },
    ],
  },
  {
    path: '/system/message',
    name: 'SystemMessage',
    meta: {
      title: '信息中心',
      icon: 'lucide:mail',
      order: 12,
    },
    children: [
      {
        path: '/system/notify-template',
        component: () => import('#/views/system/notify/template/index.vue'),
        name: 'SystemNotifyTemplate',
        meta: {
          title: '站内信模板',
          icon: 'lucide:mail-plus',
        },
      },
      {
        path: '/system/notify-message-list',
        component: () => import('#/views/system/notify/message/index.vue'),
        name: 'SystemNotifyMessageList',
        meta: {
          title: '站内信消息',
          icon: 'lucide:mails',
        },
      },
      {
        path: '/system/mail-account',
        component: () => import('#/views/system/mail/account/index.vue'),
        name: 'SystemMailAccount',
        meta: {
          title: '邮箱账号',
          icon: 'lucide:mail-check',
        },
      },
      {
        path: '/system/mail-template',
        component: () => import('#/views/system/mail/template/index.vue'),
        name: 'SystemMailTemplate',
        meta: {
          title: '邮件模板',
          icon: 'lucide:file-text',
        },
      },
      {
        path: '/system/mail-log',
        component: () => import('#/views/system/mail/log/index.vue'),
        name: 'SystemMailLog',
        meta: {
          title: '邮件日志',
          icon: 'lucide:file-clock',
        },
      },
      {
        path: '/system/sms-channel',
        component: () => import('#/views/system/sms/channel/index.vue'),
        name: 'SystemSmsChannel',
        meta: {
          title: '短信渠道',
          icon: 'lucide:message-square-more',
        },
      },
      {
        path: '/system/sms-template',
        component: () => import('#/views/system/sms/template/index.vue'),
        name: 'SystemSmsTemplate',
        meta: {
          title: '短信模板',
          icon: 'lucide:message-square-text',
        },
      },
      {
        path: '/system/sms-log',
        component: () => import('#/views/system/sms/log/index.vue'),
        name: 'SystemSmsLog',
        meta: {
          title: '短信日志',
          icon: 'lucide:message-square-warning',
        },
      },
    ],
  },
  {
    path: '/system/notify-message',
    component: () => import('#/views/system/notify/my/index.vue'),
    name: 'MyNotifyMessage',
    meta: {
      title: '我的站内信',
      icon: 'ant-design:message-filled',
      hideInMenu: true,
    },
  },
];

export default routes;
