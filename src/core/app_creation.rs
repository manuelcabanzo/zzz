use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use crate::core::file_system::FileSystem;
use crate::core::android_resources::AndroidResources;

pub struct AppCreation {
    pub app_name: String,
    pub app_path: String,
    pub api_level: String,
    resources: AndroidResources,
    logger: Rc<dyn Fn(String)>, // Add logger
    progress_callback: Rc<dyn Fn(f32)>, // Add progress callback
}

impl AppCreation {
    pub fn new(app_name: String, app_path: String, api_level: String, logger: Rc<dyn Fn(String)>, progress_callback: Rc<dyn Fn(f32)>) -> Self {
        let resources = AndroidResources::load_state()
            .unwrap_or_else(|_| AndroidResources::new());
        
        Self {
            app_name,
            app_path,
            api_level,
            resources,
            logger,
            progress_callback,
        }
    }

    pub fn create_app(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.app_name.is_empty() || self.app_path.is_empty() {
            return Ok(());
        }

        (self.logger)("Starting app creation...".to_string());
    
        // Ensure we have necessary Gradle files
        self.resources.ensure_gradle_files()?;
        (self.progress_callback)(0.1);
    
        // Ensure we have the requested API level
        self.resources.ensure_api_level(&self.api_level)?;
        (self.progress_callback)(0.2);
    
        let app_dir = PathBuf::from(&self.app_path).join(&self.app_name);
        let fs = Rc::new(FileSystem::new(app_dir.to_str().unwrap()));
        fs.create_directory(&app_dir)?;
        (self.progress_callback)(0.3);
    
        // Create project structure
        let src_main_dir = app_dir.join("src").join("main");
        let src_test_dir = app_dir.join("src").join("test");
        let src_android_test_dir = app_dir.join("src").join("androidTest");
        let kotlin_dir = src_main_dir.join("kotlin").join("com").join("example").join("app");
        let res_dir = src_main_dir.join("res");
        let java_test_dir = src_test_dir.join("java").join("com").join("example").join("app");
        let java_android_test_dir = src_android_test_dir.join("java").join("com").join("example").join("app");
    
        // Create all necessary directories
        for (i, dir) in [
            &src_main_dir,
            &src_test_dir,
            &src_android_test_dir,
            &kotlin_dir,
            &res_dir,
            &java_test_dir,
            &java_android_test_dir,
            &res_dir.join("layout"),
            &res_dir.join("values"),
            &res_dir.join("values-night"),
            &res_dir.join("xml"),
        ].iter().enumerate() {
            fs.create_directory(dir)?;
            (self.progress_callback)(0.3 + 0.05 * i as f32);
        }
    
        // Copy Gradle files from resources
        let gradle_source = self.resources.get_gradle_path();
        for file in &["gradlew", "gradlew.bat"] {
            let source = gradle_source.join(file);
            let dest = app_dir.join(file);
            fs::copy(source, dest)?;
            
            // Make gradlew executable on Unix-like systems
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&dest)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&dest, perms)?;
            }
        }
        (self.progress_callback)(0.6);
    
        // Create gradle wrapper directory and copy files
        let gradle_wrapper_dir = app_dir.join("gradle").join("wrapper");
        fs.create_directory(&gradle_wrapper_dir)?;
        
        for file in &["gradle-wrapper.jar", "gradle-wrapper.properties"] {
            let source = gradle_source.join(file);
            let dest = gradle_wrapper_dir.join(file);
            fs::copy(source, dest)?;
        }
        (self.progress_callback)(0.7);
    
        // Create build.gradle.kts
        let build_gradle_content = format!(
            r#"plugins {{
        id("com.android.application")
        id("org.jetbrains.kotlin.android")
    }}
    
    android {{
        namespace = "com.example.app"
        compileSdk = {api_level}
    
        defaultConfig {{
            applicationId = "com.example.app"
            minSdk = 24
            targetSdk = {api_level}
            versionCode = 1
            versionName = "1.0"
    
            testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
            vectorDrawables {{
                useSupportLibrary = true
            }}
        }}
    
        buildTypes {{
            release {{
                isMinifyEnabled = false
                proguardFiles(
                    getDefaultProguardFile("proguard-android-optimize.txt"),
                    "proguard-rules.pro"
                )
            }}
        }}
        
        compileOptions {{
            sourceCompatibility = JavaVersion.VERSION_17
            targetCompatibility = JavaVersion.VERSION_17
        }}
        
        kotlinOptions {{
            jvmTarget = "17"
        }}
        
        buildFeatures {{
            compose = true
        }}
        
        composeOptions {{
            kotlinCompilerExtensionVersion = "1.5.1"
        }}
        
        packaging {{
            resources {{
                excludes += "/META-INF/{{AL2.0,LGPL2.1}}"
            }}
        }}
    }}
    
    dependencies {{
        implementation("androidx.core:core-ktx:1.12.0")
        implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.7.0")
        implementation("androidx.activity:activity-compose:1.8.2")
        implementation(platform("androidx.compose:compose-bom:2023.08.00"))
        implementation("androidx.compose.ui:ui")
        implementation("androidx.compose.ui:ui-graphics")
        implementation("androidx.compose.ui:ui-tooling-preview")
        implementation("androidx.compose.material3:material3")
        testImplementation("junit:junit:4.13.2")
        androidTestImplementation("androidx.test.ext:junit:1.1.5")
        androidTestImplementation("androidx.test.espresso:espresso-core:3.5.1")
        androidTestImplementation(platform("androidx.compose:compose-bom:2023.08.00"))
        androidTestImplementation("androidx.compose.ui:ui-test-junit4")
        debugImplementation("androidx.compose.ui:ui-tooling")
        debugImplementation("androidx.compose.ui:ui-test-manifest")
    }}
    "#,
            api_level = self.api_level
        );
        fs::write(app_dir.join("build.gradle.kts"), build_gradle_content)?;
        (self.progress_callback)(0.8);
    
        // Create settings.gradle.kts
        let settings_gradle_content = r#"pluginManagement {
        repositories {
            google()
            mavenCentral()
            gradlePluginPortal()
        }
    }
    dependencyResolutionManagement {
        repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
        repositories {
            google()
            mavenCentral()
        }
    }
    
    rootProject.name = "MyApplication"
    include(":app")
    "#;
        fs::write(app_dir.join("settings.gradle.kts"), settings_gradle_content)?;
    
        // Create MainActivity.kt
        let main_activity_content = r#"package com.example.app
    
    import android.os.Bundle
    import androidx.activity.ComponentActivity
    import androidx.activity.compose.setContent
    import androidx.compose.foundation.layout.fillMaxSize
    import androidx.compose.material3.MaterialTheme
    import androidx.compose.material3.Surface
    import androidx.compose.material3.Text
    import androidx.compose.runtime.Composable
    import androidx.compose.ui.Modifier
    import androidx.compose.ui.tooling.preview.Preview
    
    class MainActivity : ComponentActivity() {
        override fun onCreate(savedInstanceState: Bundle?) {
            super.onCreate(savedInstanceState)
            setContent {
                AppTheme {
                    Surface(
                        modifier = Modifier.fillMaxSize(),
                        color = MaterialTheme.colorScheme.background
                    ) {
                        Greeting("Android")
                    }
                }
            }
        }
    }
    
    @Composable
    fun Greeting(name: String, modifier: Modifier = Modifier) {
        Text(
            text = "Hello $name!",
            modifier = modifier
        )
    }
    
    @Preview(showBackground = true)
    @Composable
    fun GreetingPreview() {
        AppTheme {
            Greeting("Android")
        }
    }
    "#;
        fs::write(kotlin_dir.join("MainActivity.kt"), main_activity_content)?;
    
        // Create Theme.kt
        let theme_content = r#"package com.example.app
    
    import android.app.Activity
    import android.os.Build
    import androidx.compose.foundation.isSystemInDarkTheme
    import androidx.compose.material3.MaterialTheme
    import androidx.compose.material3.darkColorScheme
    import androidx.compose.material3.dynamicDarkColorScheme
    import androidx.compose.material3.dynamicLightColorScheme
    import androidx.compose.material3.lightColorScheme
    import androidx.compose.runtime.Composable
    import androidx.compose.runtime.SideEffect
    import androidx.compose.ui.graphics.toArgb
    import androidx.compose.ui.platform.LocalContext
    import androidx.compose.ui.platform.LocalView
    import androidx.core.view.WindowCompat
    
    private val DarkColorScheme = darkColorScheme()
    private val LightColorScheme = lightColorScheme()
    
    @Composable
    fun AppTheme(
        darkTheme: Boolean = isSystemInDarkTheme(),
        dynamicColor: Boolean = true,
        content: @Composable () -> Unit
    ) {
        val colorScheme = when {
            dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
                val context = LocalContext.current
                if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
            }
            darkTheme -> DarkColorScheme
            else -> LightColorScheme
        }
        val view = LocalView.current
        if (!view.isInEditMode) {
            SideEffect {
                val window = (view.context as Activity).window
                window.statusBarColor = colorScheme.primary.toArgb()
                WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = darkTheme
            }
        }
    
        MaterialTheme(
            colorScheme = colorScheme,
            content = content
        )
    }
    "#;
        fs::write(kotlin_dir.join("Theme.kt"), theme_content)?;
    
        // Create AndroidManifest.xml
        let manifest_content = r#"<?xml version="1.0" encoding="utf-8"?>
    <manifest xmlns:android="http://schemas.android.com/apk/res/android"
        xmlns:tools="http://schemas.android.com/tools">
    
        <application
            android:allowBackup="true"
            android:dataExtractionRules="@xml/data_extraction_rules"
            android:fullBackupContent="@xml/backup_rules"
            android:icon="@mipmap/ic_launcher"
            android:label="@string/app_name"
            android:roundIcon="@mipmap/ic_launcher_round"
            android:supportsRtl="true"
            android:theme="@style/Theme.App"
            tools:targetApi="31">
            <activity
                android:name=".MainActivity"
                android:exported="true"
                android:theme="@style/Theme.App">
                <intent-filter>
                    <action android:name="android.intent.action.MAIN" />
                    <category android:name="android.intent.category.LAUNCHER" />
                </intent-filter>
            </activity>
        </application>
    </manifest>"#;
        fs::write(src_main_dir.join("AndroidManifest.xml"), manifest_content)?;
    
        // Create strings.xml
        let strings_content = format!(r#"<?xml version="1.0" encoding="utf-8"?>
    <resources>
        <string name="app_name">{}</string>
    </resources>"#, self.app_name);
        fs::write(res_dir.join("values").join("strings.xml"), strings_content)?;
    
        // Create themes.xml
        let themes_content = r#"<?xml version="1.0" encoding="utf-8"?>
    <resources>
        <style name="Theme.App" parent="android:Theme.Material.Light.NoActionBar" />
    </resources>"#;
        fs::write(res_dir.join("values").join("themes.xml"), themes_content)?;
    
        // Create backup_rules.xml
        let backup_rules_content = r#"<?xml version="1.0" encoding="utf-8"?>
    <full-backup-content>
        <include domain="sharedpref" path="."/>
        <exclude domain="sharedpref" path="device.xml"/>
    </full-backup-content>"#;
        fs::write(res_dir.join("xml").join("backup_rules.xml"), backup_rules_content)?;
    
        // Create data_extraction_rules.xml
        let data_extraction_content = r#"<?xml version="1.0" encoding="utf-8"?>
    <data-extraction-rules>
        <cloud-backup>
            <include domain="sharedpref" path="."/>
            <exclude domain="sharedpref" path="device.xml"/>
        </cloud-backup>
        <device-transfer>
            <include domain="sharedpref" path="."/>
            <exclude domain="sharedpref" path="device.xml"/>
        </device-transfer>
    </data-extraction-rules>"#;
        fs::write(res_dir.join("xml").join("data_extraction_rules.xml"), data_extraction_content)?;
    
        // Create ExampleUnitTest.kt
        let unit_test_content = r#"package com.example.app
    
    import org.junit.Test
    import org.junit.Assert.*
    
    class ExampleUnitTest {
        @Test
        fun addition_isCorrect() {
            assertEquals(4, 2 + 2)
        }
    }"#;
        fs::write(java_test_dir.join("ExampleUnitTest.kt"), unit_test_content)?;
    
        // Create ExampleInstrumentedTest.kt
        let instrumented_test_content = r#"package com.example.app
    
    import androidx.test.platform.app.InstrumentationRegistry
    import androidx.test.ext.junit.runners.AndroidJUnit4
    import org.junit.Test
    import org.junit.runner.RunWith
    import org.junit.Assert.*
    
    @RunWith(AndroidJUnit4::class)
    class ExampleInstrumentedTest {
        @Test
        fun useAppContext() {
            val appContext = InstrumentationRegistry.getInstrumentation().targetContext
            assertEquals("com.example.app", appContext.packageName)
        }
    }"#;
        fs::write(java_android_test_dir.join("ExampleInstrumentedTest.kt"), instrumented_test_content)?;
    
        // Create .gitignore
        let gitignore_content = r#"*.iml
    .gradle
    /local.properties
    /.idea
    .DS_Store
    /build
    /captures
    .externalNativeBuild
    .cxx
    local.properties"#;
        fs::write(app_dir.join(".gitignore"), gitignore_content)?;
    
        // Save resources state
        self.resources.save_state()?;
        (self.progress_callback)(1.0);
        (self.logger)("App creation completed.".to_string());
    
        Ok(())
    }

}

