import { createRouter, createWebHashHistory } from 'vue-router'

export const router = createRouter({
  routes: [
    {
      path: '/',
      name: 'launchpad',
      component: () => import('@/pages/launchpad/index.vue'),
    },
    {
      path: '/circle/:id',
      name: 'circle-room',
      component: () => import('@/pages/circle-space/index.vue'),
    },
    {
      path: '/direct/:id',
      name: 'direct-room',
      component: () => import('@/pages/direct-space/index.vue'),
    },
    {
      path: '/workbench',
      name: 'workbench',
      component: () => import('@/pages/workbench/index.vue'),
    },
  ],
  history: createWebHashHistory(),
})

router.beforeEach((_to, _from, next) => {
  next()
})

router.afterEach((to) => {
  document.body.id = `page-${to.name as string}`
})

/** Warm the most common room views after the router becomes idle-ready. */
function preloadCriticalRoutes() {
  requestIdleCallback(() => {
    // English engineering note.
    import('@/pages/circle-space/index.vue')
    import('@/pages/direct-space/index.vue')
  })
}

// English engineering note.
router.isReady().then(preloadCriticalRoutes)
