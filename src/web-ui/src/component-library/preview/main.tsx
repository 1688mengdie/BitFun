/**
 * Component preview entry
 */

import React from 'react';
import ReactDOM from 'react-dom/client';
import { PreviewApp } from './PreviewApp';
import './preview.css';
import './flowchat-cards-preview.css';

import '../../app/styles/index.scss';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <PreviewApp />
  </React.StrictMode>
);