/**
 * 3D Rubik's Cube Component
 * Interactive cube animation implemented with Three.js
 * Supports dark/light theme adaptation
 */

import { useEffect, useRef, useState, useCallback } from 'react';
import * as THREE from 'three';

/**
 * Detect current theme type
 */
function getThemeType(): 'dark' | 'light' {
  if (typeof document === 'undefined') return 'dark';
  
  const themeType = document.documentElement.getAttribute('data-theme-type');
  if (themeType === 'light' || themeType === 'dark') {
    return themeType;
  }
  
  const dataTheme = document.documentElement.getAttribute('data-theme');
  if (dataTheme?.includes('light')) return 'light';
  if (dataTheme?.includes('dark')) return 'dark';
  
  if (document.documentElement.classList.contains('light')) return 'light';
  
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

const DARK_THEME_COLORS = {
  cubeFaces: [
    { color: 0x3a3a42, opacity: 0.4 },
    { color: 0x2d2d35, opacity: 0.35 },
    { color: 0x42424a, opacity: 0.45 },
    { color: 0x252530, opacity: 0.3 },
    { color: 0x35353d, opacity: 0.4 },
    { color: 0x2a2a32, opacity: 0.35 },
  ],
  edgeColor: 0x64b4ff,
  edgeOpacity: 0.15,
  hoverEdgeOpacity: 0.6,
};

const LIGHT_THEME_COLORS = {
  cubeFaces: [
    { color: 0x94a3b8, opacity: 0.28 },  // slate-400, opacity 0.28 (front)
    { color: 0x94a3b8, opacity: 0.18 },  // slate-400, opacity 0.18 (back)
    { color: 0x94a3b8, opacity: 0.38 },  // slate-400, opacity 0.38 (top)
    { color: 0x94a3b8, opacity: 0.14 },  // slate-400, opacity 0.14 (bottom)
    { color: 0x94a3b8, opacity: 0.24 },  // slate-400, opacity 0.24 (right)
    { color: 0x94a3b8, opacity: 0.20 },  // slate-400, opacity 0.20 (left)
  ],
  edgeColor: 0x94a3b8,
  edgeOpacity: 0.30,
  hoverEdgeOpacity: 0.6,
};

export default function RubiksCube3D() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [themeType, setThemeType] = useState<'dark' | 'light'>(getThemeType);
  const sceneRef = useRef<{
    scene: THREE.Scene;
    camera: THREE.PerspectiveCamera;
    renderer: THREE.WebGLRenderer;
    cubes: THREE.Group[];
    cubeGroup: THREE.Group;
    animationId: number;
    raycaster: THREE.Raycaster;
    mouse: THREE.Vector2;
    hoveredCube: THREE.Group | null;
    handleMouseMove: (event: MouseEvent) => void;
    handleResize: () => void;
  } | null>(null);

  useEffect(() => {
    const checkTheme = () => {
      const newTheme = getThemeType();
      setThemeType(newTheme);
    };

    const observer = new MutationObserver(checkTheme);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme', 'data-theme-type', 'class'],
    });

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    mediaQuery.addEventListener('change', checkTheme);

    return () => {
      observer.disconnect();
      mediaQuery.removeEventListener('change', checkTheme);
    };
  }, []);

  const updateCubeColors = useCallback((isDark: boolean) => {
    if (!sceneRef.current) return;

    const colors = isDark ? DARK_THEME_COLORS : LIGHT_THEME_COLORS;
    const { cubes } = sceneRef.current;

    cubes.forEach((cubeUnit) => {
      const mesh = cubeUnit.children[0] as THREE.Mesh;
      if (mesh && Array.isArray(mesh.material)) {
        mesh.material.forEach((mat, i) => {
          mat.color.setHex(colors.cubeFaces[i].color);
          mat.opacity = colors.cubeFaces[i].opacity;
        });
      }

      const edges = cubeUnit.children[1] as THREE.LineSegments;
      if (edges) {
        const edgeMat = edges.material as THREE.LineBasicMaterial;
        edgeMat.color.setHex(colors.edgeColor);
        if (!cubeUnit.userData.isHovered) {
          edgeMat.opacity = colors.edgeOpacity;
        }
        cubeUnit.userData.baseEdgeOpacity = colors.edgeOpacity;
        cubeUnit.userData.baseOpacities = colors.cubeFaces.map(f => f.opacity);
      }
    });
  }, []);

  useEffect(() => {
    updateCubeColors(themeType === 'dark');
  }, [themeType, updateCubeColors]);

  useEffect(() => {
    if (!containerRef.current) return;

    const container = containerRef.current;
    let cleanupCalled = false;
    let initFrameId: number | null = null;
    const isDark = themeType === 'dark';
    const colors = isDark ? DARK_THEME_COLORS : LIGHT_THEME_COLORS;

    const initScene = () => {
      if (cleanupCalled) return;
      
      const width = container.clientWidth;
      const height = container.clientHeight;
      
      if (width === 0 || height === 0) {
        initFrameId = requestAnimationFrame(initScene);
        return;
      }

      const scene = new THREE.Scene();

      const camera = new THREE.PerspectiveCamera(45, width / height, 0.1, 1000);
      camera.position.set(1.5, 0.3, 14);

      const renderer = new THREE.WebGLRenderer({
        antialias: true,
        alpha: true,
      });
      renderer.setSize(width, height);
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      container.appendChild(renderer.domElement);

      const raycaster = new THREE.Raycaster();
      const mouse = new THREE.Vector2();

      const cubeGroup = new THREE.Group();
      scene.add(cubeGroup);

      const createMaterials = () => colors.cubeFaces.map(face => 
        new THREE.MeshBasicMaterial({ 
          color: face.color, 
          transparent: true, 
          opacity: face.opacity 
        })
      );

      const createEdgeMaterial = () => new THREE.LineBasicMaterial({
        color: colors.edgeColor,
        opacity: colors.edgeOpacity,
        transparent: true,
      });

      const cubes: THREE.Group[] = [];
      const positions = [-1, 0, 1];
      const cubeSize = 0.75;
      const gap = 0.85;

      positions.forEach(x => {
        positions.forEach(y => {
          positions.forEach(z => {
            const cubeUnit = new THREE.Group();

            const geometry = new THREE.BoxGeometry(cubeSize, cubeSize, cubeSize);
            const cubeMaterials = createMaterials();
            const cube = new THREE.Mesh(geometry, cubeMaterials);
            cubeUnit.add(cube);

            const edgeGeometry = new THREE.EdgesGeometry(geometry);
            const edgeMaterial = createEdgeMaterial();
            const edges = new THREE.LineSegments(edgeGeometry, edgeMaterial);
            cubeUnit.add(edges);

            cubeUnit.position.set(x * gap, y * gap, z * gap);
            
            cubeUnit.userData = { 
              baseX: x * gap, 
              baseY: y * gap, 
              baseZ: z * gap,
              dirX: x,
              dirY: y,
              dirZ: z,
              baseOpacities: cubeMaterials.map(m => m.opacity),
              baseEdgeOpacity: colors.edgeOpacity,
              isHovered: false,
            };

            cubeGroup.add(cubeUnit);
            cubes.push(cubeUnit);
          });
        });
      });

      cubeGroup.rotation.x = -0.35;
      cubeGroup.rotation.y = -0.8;

      let hoveredCube: THREE.Group | null = null;

      const handleMouseMove = (event: MouseEvent) => {
        const rect = container.getBoundingClientRect();
        mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
        mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
      };

      container.addEventListener('mousemove', handleMouseMove);

      let angle = -0.8;
      const animate = () => {
        if (cleanupCalled) return;
        
        angle += 0.003;
        cubeGroup.rotation.y = angle;

        raycaster.setFromCamera(mouse, camera);
        const meshes = cubes.map(c => c.children[0] as THREE.Mesh);
        const intersects = raycaster.intersectObjects(meshes);

        if (hoveredCube && (!intersects.length || intersects[0].object.parent !== hoveredCube)) {
          const { baseEdgeOpacity } = hoveredCube.userData;
          hoveredCube.userData.isHovered = false;
          const edges = hoveredCube.children[1] as THREE.LineSegments;
          (edges.material as THREE.LineBasicMaterial).opacity = baseEdgeOpacity;
          (edges.material as THREE.LineBasicMaterial).color.setHex(colors.edgeColor);
          hoveredCube = null;
        }

        if (intersects.length > 0) {
          const newHovered = intersects[0].object.parent as THREE.Group;
          if (newHovered !== hoveredCube) {
            hoveredCube = newHovered;
            hoveredCube.userData.isHovered = true;
            const edges = hoveredCube.children[1] as THREE.LineSegments;
            (edges.material as THREE.LineBasicMaterial).opacity = colors.hoverEdgeOpacity;
            (edges.material as THREE.LineBasicMaterial).color.setHex(colors.edgeColor);
          }
        }

        renderer.render(scene, camera);
        sceneRef.current!.animationId = requestAnimationFrame(animate);
      };

      const handleResize = () => {
        const w = container.clientWidth;
        const h = container.clientHeight;
        if (w === 0 || h === 0) return;
        camera.aspect = w / h;
        camera.updateProjectionMatrix();
        renderer.setSize(w, h);
      };
      window.addEventListener('resize', handleResize);

      sceneRef.current = {
        scene,
        camera,
        renderer,
        cubes,
        cubeGroup,
        animationId: 0,
        raycaster,
        mouse,
        hoveredCube: null,
        handleMouseMove,
        handleResize,
      };

      animate();
    };

    initFrameId = requestAnimationFrame(initScene);

    return () => {
      cleanupCalled = true;
      if (initFrameId !== null) {
        cancelAnimationFrame(initFrameId);
      }
      if (sceneRef.current) {
        cancelAnimationFrame(sceneRef.current.animationId);
        window.removeEventListener('resize', sceneRef.current.handleResize);
        container.removeEventListener('mousemove', sceneRef.current.handleMouseMove);
        sceneRef.current.renderer.dispose();
        if (sceneRef.current.renderer.domElement.parentNode === container) {
          container.removeChild(sceneRef.current.renderer.domElement);
        }
        sceneRef.current = null;
      }
    };
  }, []);

  return (
    <div
      ref={containerRef}
      className="startup-cube-container"
      style={{
        width: '100%',
        height: '100%',
        position: 'absolute',
        inset: 0,
      }}
    />
  );
}
