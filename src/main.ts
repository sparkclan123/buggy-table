import { mount } from 'svelte';
import App from './App.svelte';
import './styles.css';
import { initializeTheme } from './lib/theme';

initializeTheme();

mount(App, { target: document.getElementById('app')! });
