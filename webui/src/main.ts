import { createApp } from 'vue'
import 'vue-sonner/style.css'

import App from './App.vue'
import { i18n } from './lib/i18n'
import './style.css'

createApp(App).use(i18n).mount('#app')
